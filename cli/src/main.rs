mod commands;
mod eps;
mod state;
mod tailscale;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "epc", about = "Extremely Personal Cloud — EPS service runtime", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check all running services for insecure network bindings
    Audit,
    /// Start an EPS service from a project directory (or installed package).
    /// Run from inside a project directory with no arguments to serve it locally.
    #[command(name = "serve")]
    Serve {
        /// Package name (looks up in ~/.epm/packages/). Omit when inside a project
        /// directory — EPC will detect the eps.toml and serve it automatically.
        spec: Option<String>,
        /// Path to a local EPS directory (skips epm lookup).
        /// Defaults to the current directory if it contains an eps.toml.
        #[arg(long)]
        local: Option<std::path::PathBuf>,
    },
    /// List running services with their ports and Tailscale URLs
    Ps,
    /// Tail logs for a running service
    Logs {
        /// Service name
        name: String,
    },
    /// Stop a running service
    Stop {
        /// Service name
        name: String,
    },
    /// Fully remove a service: stop it, delete its log, and purge it from the Observatory database
    Remove {
        /// Service name
        name: String,
    },
    /// Remove all services whose project directory no longer exists
    Prune,
    /// Stop and restart a running service (picks up source changes)
    Restart {
        /// Service name
        name: String,
    },
    /// Restart all services registered in ~/.epc/services.toml that are not already running.
    /// Waits for Tailscale to be ready before deploying. Run automatically by the login
    /// LaunchAgent installed via `epc install-startup`.
    Startup,
    /// Install a macOS LaunchAgent so EPC services restart automatically on login.
    /// Creates ~/Library/LaunchAgents/com.eps.epc-startup.plist and loads it.
    /// macOS only.
    #[cfg(target_os = "macos")]
    InstallStartup,
    /// Manage the Observatory monitoring database
    Observatory {
        #[command(subcommand)]
        command: ObservatoryCommands,
    },
    /// Update epc to the latest release
    SelfUpdate,
}

#[derive(Subcommand)]
enum ObservatoryCommands {
    /// Remove one or more stale service entries from the Observatory database.
    ///
    /// Services that have been stopped or deleted are never automatically pruned
    /// from Observatory's SQLite history — use this to clean them up.
    ///
    /// Example:
    ///   epc observatory rm mirror epc
    Rm {
        /// One or more service names to remove
        #[arg(required = true)]
        names: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Audit => commands::audit::run().await?,
        Commands::Serve { spec, local } => commands::serve::run(spec.as_deref(), local.as_deref()).await?,
        Commands::Ps => commands::ps::run().await?,
        Commands::Logs { name } => commands::logs::run(name).await?,
        Commands::Stop { name } => commands::stop::run(name)?,
        Commands::Remove { name } => commands::remove::run(name)?,
        Commands::Prune => commands::prune::run()?,
        Commands::Restart { name } => commands::restart::run(name).await?,
        Commands::Startup => commands::startup::run().await?,
        #[cfg(target_os = "macos")]
        Commands::InstallStartup => commands::install_startup::run()?,
        Commands::Observatory { command } => match command {
            ObservatoryCommands::Rm { names } => commands::observatory::run(names)?,
        },
        Commands::SelfUpdate => commands::self_update::run().await?,
    }

    Ok(())
}
