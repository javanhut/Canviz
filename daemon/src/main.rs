mod config;
mod daemon;
mod hyprland;
mod image;
mod ipc;
mod render;
mod surface;

use clap::Parser;
use color_eyre::eyre::Result;
use log::{error, info};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "canviz")]
#[command(author, version, about = "Modern wallpaper daemon for Hyprland", long_about = None)]
struct Args {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Run in verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// Run in foreground (don't daemonize)
    #[arg(short, long)]
    foreground: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    info!("Starting Canviz wallpaper daemon v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config_path = args.config.unwrap_or_else(|| {
        dirs::config_dir()
            .map(|p| p.join("canviz/config.toml"))
            .expect("Could not determine config directory")
    });

    info!("Loading config from: {:?}", config_path);

    let config = match config::Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load config: {}", e);
            error!("Using default configuration");
            config::Config::default()
        }
    };

    // Run the daemon
    if let Err(e) = daemon::run(config, args.foreground) {
        error!("Daemon error: {:?}", e);
        return Err(e);
    }

    Ok(())
}
