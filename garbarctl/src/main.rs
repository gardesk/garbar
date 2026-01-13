use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "garbarctl")]
#[command(author, version, about = "Control tool for garbar")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the bar
    Show,
    /// Hide the bar
    Hide,
    /// Toggle bar visibility
    Toggle,
    /// Reload configuration
    Reload,
    /// Get bar status
    Status,
    /// Force update a specific module
    Update {
        /// Module name to update
        module: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // TODO: Implement IPC client
    match cli.command {
        Commands::Show => {
            println!("garbarctl show: not yet implemented");
        }
        Commands::Hide => {
            println!("garbarctl hide: not yet implemented");
        }
        Commands::Toggle => {
            println!("garbarctl toggle: not yet implemented");
        }
        Commands::Reload => {
            println!("garbarctl reload: not yet implemented");
        }
        Commands::Status => {
            println!("garbarctl status: not yet implemented");
        }
        Commands::Update { module } => {
            println!("garbarctl update {}: not yet implemented", module);
        }
    }

    Ok(())
}
