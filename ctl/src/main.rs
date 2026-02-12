use clap::{Parser, Subcommand};
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// IPC Commands (must match daemon's IPC module)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum IpcCommand {
    Status,
    Set {
        monitor: Option<String>,
        path: PathBuf,
    },
    Next {
        monitor: Option<String>,
    },
    Previous {
        monitor: Option<String>,
    },
    Reload,
    GetWallpaper {
        monitor: Option<String>,
    },
    Pause {
        monitor: Option<String>,
    },
    Resume {
        monitor: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum IpcResponse {
    Ok { message: Option<String> },
    Error { message: String },
    Status { monitors: Vec<MonitorStatus> },
    Wallpaper { path: Option<PathBuf> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStatus {
    pub name: String,
    pub wallpaper: Option<PathBuf>,
    pub workspace: Option<i32>,
    pub slideshow_active: bool,
    pub slideshow_paused: bool,
}

/// Control tool for Canviz wallpaper daemon
#[derive(Parser, Debug)]
#[command(name = "canvizctl")]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show status of all monitors
    Status,

    /// Set wallpaper for a monitor
    Set {
        /// Path to wallpaper image
        path: PathBuf,

        /// Monitor name (all monitors if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },

    /// Switch to next wallpaper in slideshow
    Next {
        /// Monitor name (all monitors if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },

    /// Switch to previous wallpaper in slideshow
    Previous {
        /// Monitor name (all monitors if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },

    /// Reload configuration from file
    Reload,

    /// Get current wallpaper path for a monitor
    Get {
        /// Monitor name (first monitor if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },

    /// Pause slideshow
    Pause {
        /// Monitor name (all monitors if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },

    /// Resume slideshow
    Resume {
        /// Monitor name (all monitors if not specified)
        #[arg(short, long)]
        monitor: Option<String>,
    },
}

fn socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());

    let uid = unsafe { libc::getuid() };
    Ok(PathBuf::from(format!("{}/canviz-{}.sock", runtime_dir, uid)))
}

async fn send_command(command: IpcCommand) -> Result<IpcResponse> {
    let socket = socket_path()?;

    let mut stream = UnixStream::connect(&socket)
        .await
        .wrap_err_with(|| {
            format!(
                "Failed to connect to canviz daemon at {:?}\nIs the daemon running?",
                socket
            )
        })?;

    // Send command
    let json = serde_json::to_vec(&command).wrap_err("Failed to serialize command")?;
    stream
        .write_all(&json)
        .await
        .wrap_err("Failed to send command")?;

    // Read response
    let mut buf = vec![0u8; 8192];
    let n = stream
        .read(&mut buf)
        .await
        .wrap_err("Failed to read response")?;

    let response: IpcResponse =
        serde_json::from_slice(&buf[..n]).wrap_err("Failed to parse response")?;

    Ok(response)
}

fn print_status(monitors: &[MonitorStatus]) {
    println!("Canviz Status");
    println!("{}", "=".repeat(60));

    for monitor in monitors {
        println!("\nMonitor: {}", monitor.name);
        println!(
            "  Wallpaper: {}",
            monitor
                .wallpaper
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "None".to_string())
        );
        if let Some(ws) = monitor.workspace {
            println!("  Workspace: {}", ws);
        }
        println!(
            "  Slideshow: {}",
            if monitor.slideshow_active {
                if monitor.slideshow_paused {
                    "paused"
                } else {
                    "running"
                }
            } else {
                "disabled"
            }
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let command = match args.command {
        Commands::Status => IpcCommand::Status,
        Commands::Set { path, monitor } => IpcCommand::Set {
            monitor,
            path: path.canonicalize().unwrap_or(path),
        },
        Commands::Next { monitor } => IpcCommand::Next { monitor },
        Commands::Previous { monitor } => IpcCommand::Previous { monitor },
        Commands::Reload => IpcCommand::Reload,
        Commands::Get { monitor } => IpcCommand::GetWallpaper { monitor },
        Commands::Pause { monitor } => IpcCommand::Pause { monitor },
        Commands::Resume { monitor } => IpcCommand::Resume { monitor },
    };

    let response = send_command(command).await?;

    match response {
        IpcResponse::Ok { message } => {
            if let Some(msg) = message {
                println!("{}", msg);
            } else {
                println!("OK");
            }
        }
        IpcResponse::Error { message } => {
            eprintln!("Error: {}", message);
            std::process::exit(1);
        }
        IpcResponse::Status { monitors } => {
            print_status(&monitors);
        }
        IpcResponse::Wallpaper { path } => {
            if let Some(p) = path {
                println!("{}", p.display());
            } else {
                println!("No wallpaper set");
            }
        }
    }

    Ok(())
}
