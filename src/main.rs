mod api;
mod cache;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    let token = cfg
        .token
        .ok_or_else(|| anyhow::anyhow!("Not configured. Run 'slack config --token <TOKEN>' first"))?;
    api::Client::new(&token)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config { token } => {
            let cfg = config::Config {
                token: Some(token),
            };
            config::save_config(&cfg)?;
            eprintln!("Configuration saved");
        }
        Commands::Channels { filter } => {
            let client = get_client()?;
            let result = client.get_channels_cached().await?;
            if let Some(filter) = filter {
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
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        Commands::Channel { id } => {
            let client = get_client()?;
            let result = client.get_channel(&id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Messages { channel, limit, grep, ts } => {
            let client = get_client()?;
            let result = client.get_messages(&channel, limit).await?;
            if grep.is_some() || ts.is_some() {
                if let Some(messages) = result.get("messages").and_then(|m| m.as_array()) {
                    let filtered: Vec<_> = messages
                        .iter()
                        .filter(|msg| {
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
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        Commands::Send { target, text, thread } => {
            let client = get_client()?;
            let resolved = client.resolve_target(&target).await?;
            let result = client.send_message(&resolved, &text, thread.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Dms => {
            let client = get_client()?;
            let result = client.list_dms().await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Users => {
            let client = get_client()?;
            let result = client.list_users().await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Search { query, channel, limit } => {
            let client = get_client()?;
            let full_query = match channel {
                Some(ch) => format!("{} in:#{}", query, ch.trim_start_matches('#')),
                None => query,
            };
            let result = client.search_messages(&full_query, limit).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
