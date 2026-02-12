use color_eyre::eyre::{Result, WrapErr};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// IPC Commands that can be sent to the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum IpcCommand {
    /// Get current status
    Status,
    /// Set wallpaper for a monitor
    Set {
        monitor: Option<String>,
        path: PathBuf,
    },
    /// Go to next wallpaper (slideshow)
    Next { monitor: Option<String> },
    /// Go to previous wallpaper (slideshow)
    Previous { monitor: Option<String> },
    /// Reload configuration
    Reload,
    /// Get current wallpaper for a monitor
    GetWallpaper { monitor: Option<String> },
    /// Pause slideshow
    Pause { monitor: Option<String> },
    /// Resume slideshow
    Resume { monitor: Option<String> },
}

/// IPC Response from the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum IpcResponse {
    Ok { message: Option<String> },
    Error { message: String },
    Status { monitors: Vec<MonitorStatus> },
    Wallpaper { path: Option<PathBuf> },
}

/// Status of a single monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStatus {
    pub name: String,
    pub wallpaper: Option<PathBuf>,
    pub workspace: Option<i32>,
    pub slideshow_active: bool,
    pub slideshow_paused: bool,
}

/// Get the IPC socket path
pub fn socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());

    let uid = unsafe { libc::getuid() };
    Ok(PathBuf::from(format!("{}/canviz-{}.sock", runtime_dir, uid)))
}

/// IPC Server for the daemon
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Create a new IPC server
    pub async fn new() -> Result<Self> {
        let socket_path = socket_path()?;

        // Remove existing socket if present
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)
                .wrap_err("Failed to remove existing socket")?;
        }

        info!("Starting IPC server at {:?}", socket_path);

        let listener = UnixListener::bind(&socket_path)
            .wrap_err("Failed to bind IPC socket")?;

        Ok(Self { listener })
    }

    /// Accept a connection and handle the command
    pub async fn accept(&self) -> Result<(IpcCommand, UnixStream)> {
        let (mut stream, _) = self.listener.accept().await
            .wrap_err("Failed to accept IPC connection")?;

        debug!("Accepted IPC connection");

        // Read the command
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await
            .wrap_err("Failed to read from IPC socket")?;

        let command: IpcCommand = serde_json::from_slice(&buf[..n])
            .wrap_err("Failed to parse IPC command")?;

        debug!("Received IPC command: {:?}", command);

        Ok((command, stream))
    }

    /// Send a response
    pub async fn respond(mut stream: UnixStream, response: IpcResponse) -> Result<()> {
        let json = serde_json::to_vec(&response)
            .wrap_err("Failed to serialize response")?;

        stream.write_all(&json).await
            .wrap_err("Failed to write response")?;

        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file
        if let Ok(path) = socket_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// IPC Client for canvizctl
pub struct IpcClient;

impl IpcClient {
    /// Send a command to the daemon and get the response
    pub async fn send(command: IpcCommand) -> Result<IpcResponse> {
        let socket_path = socket_path()?;

        let mut stream = UnixStream::connect(&socket_path).await
            .wrap_err_with(|| format!("Failed to connect to daemon at {:?}", socket_path))?;

        // Send command
        let json = serde_json::to_vec(&command)
            .wrap_err("Failed to serialize command")?;
        stream.write_all(&json).await
            .wrap_err("Failed to send command")?;

        // Read response
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await
            .wrap_err("Failed to read response")?;

        let response: IpcResponse = serde_json::from_slice(&buf[..n])
            .wrap_err("Failed to parse response")?;

        Ok(response)
    }
}
