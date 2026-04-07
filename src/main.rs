mod api;
mod cache;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "slack")]
#[command(about = "Slack CLI for reading and sending messages")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure the Slack token
    Config {
        /// Bot token (xoxb-...)
        #[arg(short, long)]
        token: String,
    },
    /// List channels
    Channels {
        /// Filter by channel name (case-insensitive substring match)
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Get channel info
    Channel {
        /// Channel ID
        id: String,
    },
    /// Read message history from a channel
    Messages {
        /// Channel ID
        channel: String,
        /// Number of messages to fetch
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Filter by text pattern (case-insensitive)
        #[arg(short, long)]
        grep: Option<String>,
        /// Filter by timestamp
        #[arg(long)]
        ts: Option<String>,
    },
    /// Read replies in a thread
    Thread {
        /// Channel ID
        channel: String,
        /// Thread timestamp (ts of the parent message)
        ts: String,
        /// Number of replies to fetch
        #[arg(short, long, default_value = "100")]
        limit: u32,
    },
    /// Send a message to a channel or user
    Send {
        /// Channel ID, channel name, user ID, or @username
        target: String,
        /// Message text
        text: String,
        /// Thread timestamp to reply to
        #[arg(short, long)]
        thread: Option<String>,
    },
    /// List DM conversations
    Dms,
    /// List workspace users
    Users,
    /// Search messages
    Search {
        /// Search query
        query: String,
        /// Limit search to channel (name or ID)
        #[arg(short, long)]
        channel: Option<String>,
        /// Number of results to return
        #[arg(short = 'n', long, default_value = "20")]
        limit: u32,
    },
}

fn get_client() -> Result<api::Client> {
    let cfg = config::load_config()?;
    let token = cfg.token.ok_or_else(|| {
        anyhow::anyhow!("Not configured. Run 'slack config --token <TOKEN>' first")
    })?;
    api::Client::new(&token)
}

fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

async fn cmd_channels(filter: Option<String>) -> Result<()> {
    let client = get_client()?;
    let result = client.get_channels_cached().await?;
    let Some(filter) = filter else {
        return print_json(&result);
    };
    let filter_lower = filter.to_lowercase();
    if let Some(channels) = result.get("channels").and_then(|c| c.as_array()) {
        let filtered: Vec<_> = channels
            .iter()
            .filter(|ch| {
                ch.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.to_lowercase().contains(&filter_lower))
                    .unwrap_or(false)
            })
            .collect();
        print_json(&serde_json::json!(filtered))?;
    }
    Ok(())
}

async fn cmd_messages(
    channel: String,
    limit: u32,
    grep: Option<String>,
    ts: Option<String>,
) -> Result<()> {
    let client = get_client()?;
    let result = client.get_messages(&channel, limit).await?;
    if grep.is_none() && ts.is_none() {
        return print_json(&result);
    }
    if let Some(messages) = result.get("messages").and_then(|m| m.as_array()) {
        let filtered: Vec<_> = messages
            .iter()
            .filter(|msg| matches_message_filters(msg, &grep, &ts))
            .collect();
        print_json(&serde_json::json!(filtered))?;
    }
    Ok(())
}

fn matches_message_filters(msg: &Value, grep: &Option<String>, ts: &Option<String>) -> bool {
    let grep_match = grep.as_ref().map_or(true, |pattern| {
        let pattern_lower = pattern.to_lowercase();
        msg.get("text")
            .and_then(|t| t.as_str())
            .map(|t| t.to_lowercase().contains(&pattern_lower))
            .unwrap_or(false)
    });
    let ts_match = ts.as_ref().map_or(true, |ts_val| {
        msg.get("ts")
            .and_then(|t| t.as_str())
            .map(|t| t == ts_val)
            .unwrap_or(false)
    });
    grep_match && ts_match
}

async fn cmd_thread(channel: String, ts: String, limit: u32) -> Result<()> {
    let client = get_client()?;
    let result = client.get_thread(&channel, &ts, limit).await?;
    print_json(&result)
}

async fn cmd_send(target: String, text: String, thread: Option<String>) -> Result<()> {
    let client = get_client()?;
    let resolved = client.resolve_target(&target).await?;
    let result = client
        .send_message(&resolved, &text, thread.as_deref())
        .await?;
    print_json(&result)
}

async fn cmd_search(query: String, channel: Option<String>, limit: u32) -> Result<()> {
    let client = get_client()?;
    let full_query = match channel {
        Some(ch) => format!("{} in:#{}", query, ch.trim_start_matches('#')),
        None => query,
    };
    let result = client.search_messages(&full_query, limit).await?;
    print_json(&result)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config { token } => {
            config::save_config(&config::Config { token: Some(token) })?;
            eprintln!("Configuration saved");
        }
        Commands::Channels { filter } => cmd_channels(filter).await?,
        Commands::Channel { id } => print_json(&get_client()?.get_channel(&id).await?)?,
        Commands::Messages {
            channel,
            limit,
            grep,
            ts,
        } => cmd_messages(channel, limit, grep, ts).await?,
        Commands::Thread { channel, ts, limit } => cmd_thread(channel, ts, limit).await?,
        Commands::Send {
            target,
            text,
            thread,
        } => cmd_send(target, text, thread).await?,
        Commands::Dms => print_json(&get_client()?.list_dms().await?)?,
        Commands::Users => print_json(&get_client()?.list_users().await?)?,
        Commands::Search {
            query,
            channel,
            limit,
        } => cmd_search(query, channel, limit).await?,
    }

    Ok(())
}
