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
    Channels,
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
    },
    /// Send a message to a channel or user
    Send {
        /// Channel ID, channel name, user ID, or @username
        target: String,
        /// Message text
        text: String,
    },
    /// List DM conversations
    Dms,
    /// List workspace users
    Users,
    /// Search messages
    Search {
        /// Search query
        query: String,
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
        Commands::Channels => {
            let client = get_client()?;
            let result = client.list_channels().await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Channel { id } => {
            let client = get_client()?;
            let result = client.get_channel(&id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Messages { channel, limit } => {
            let client = get_client()?;
            let result = client.get_messages(&channel, limit).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Send { target, text } => {
            let client = get_client()?;
            let resolved = client.resolve_target(&target).await?;
            let result = client.send_message(&resolved, &text).await?;
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
        Commands::Search { query } => {
            let client = get_client()?;
            let result = client.search_messages(&query).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
