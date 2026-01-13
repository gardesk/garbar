use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod daemon;
mod x11;

#[derive(Parser)]
#[command(name = "garbar")]
#[command(author, version, about = "A configurable status bar for the gar desktop suite")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the garbar daemon
    Daemon {
        /// Path to config file (default: ~/.config/gar/init.lua)
        #[arg(short, long)]
        config: Option<String>,

        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_logging(cli.verbose);

    match cli.command {
        Commands::Daemon { config, foreground } => {
            info!("Starting garbar daemon");

            if let Err(e) = daemon::run(config, foreground).await {
                error!("Daemon error: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
