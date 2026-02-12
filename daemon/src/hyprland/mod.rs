use color_eyre::eyre::{eyre, Result, WrapErr};
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// Hyprland workspace change event
#[derive(Debug, Clone)]
pub struct WorkspaceEvent {
    pub workspace_id: i32,
    pub workspace_name: String,
    pub monitor: String,
}

/// Hyprland monitor info
#[derive(Debug, Clone, Deserialize)]
pub struct HyprlandMonitor {
    pub id: i32,
    pub name: String,
    pub description: String,
    #[serde(rename = "activeWorkspace")]
    pub active_workspace: HyprlandWorkspace,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HyprlandWorkspace {
    pub id: i32,
    pub name: String,
}

/// Get Hyprland IPC socket paths
fn get_hyprland_socket_paths() -> Result<(PathBuf, PathBuf)> {
    let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .wrap_err("HYPRLAND_INSTANCE_SIGNATURE not set - are you running Hyprland?")?;

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".to_string());

    let socket1 = PathBuf::from(format!("{}/hypr/{}/.socket.sock", runtime_dir, instance));
    let socket2 = PathBuf::from(format!("{}/hypr/{}/.socket2.sock", runtime_dir, instance));

    Ok((socket1, socket2))
}

/// Hyprland IPC client
pub struct HyprlandClient;

impl HyprlandClient {
    /// Get list of monitors from Hyprland
    pub async fn get_monitors() -> Result<Vec<HyprlandMonitor>> {
        let (socket1, _) = get_hyprland_socket_paths()?;

        let mut stream = UnixStream::connect(&socket1).await
            .wrap_err("Failed to connect to Hyprland socket")?;

        stream.write_all(b"j/monitors").await
            .wrap_err("Failed to send monitors command")?;

        let mut response = String::new();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut response).await
            .wrap_err("Failed to read monitors response")?;

        let monitors: Vec<HyprlandMonitor> = serde_json::from_str(&response)
            .wrap_err("Failed to parse monitors response")?;

        Ok(monitors)
    }

    /// Get active workspace for a monitor
    pub async fn get_active_workspace(monitor: &str) -> Result<i32> {
        let monitors = Self::get_monitors().await?;

        for mon in monitors {
            if mon.name == monitor {
                return Ok(mon.active_workspace.id);
            }
        }

        Err(eyre!("Monitor not found: {}", monitor))
    }
}

/// Event listener for Hyprland workspace changes
pub struct WorkspaceListener {
    rx: mpsc::Receiver<WorkspaceEvent>,
}

impl WorkspaceListener {
    /// Start listening for workspace events
    pub async fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(32);

        // Spawn the event listener task
        tokio::spawn(async move {
            if let Err(e) = Self::event_loop(tx).await {
                error!("Hyprland event loop error: {:?}", e);
            }
        });

        Ok(Self { rx })
    }

    /// Main event listening loop
    async fn event_loop(tx: mpsc::Sender<WorkspaceEvent>) -> Result<()> {
        let (_, socket2) = get_hyprland_socket_paths()?;

        info!("Connecting to Hyprland event socket: {:?}", socket2);

        let stream = UnixStream::connect(&socket2).await
            .wrap_err("Failed to connect to Hyprland event socket")?;

        let reader = BufReader::new(stream);
        let mut lines = reader.lines();

        info!("Listening for Hyprland workspace events");

        while let Ok(Some(line)) = lines.next_line().await {
            debug!("Hyprland event: {}", line);

            // Parse workspace events
            // Format: workspace>>WORKSPACENAME or workspacev2>>WORKSPACEID,WORKSPACENAME
            if let Some(event) = Self::parse_event(&line) {
                if let Err(e) = tx.send(event).await {
                    warn!("Failed to send workspace event: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Parse a Hyprland event line
    fn parse_event(line: &str) -> Option<WorkspaceEvent> {
        // workspacev2>>id,name
        if let Some(data) = line.strip_prefix("workspacev2>>") {
            let parts: Vec<&str> = data.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(id) = parts[0].parse::<i32>() {
                    return Some(WorkspaceEvent {
                        workspace_id: id,
                        workspace_name: parts[1].to_string(),
                        monitor: String::new(), // Will be determined separately
                    });
                }
            }
        }

        // activespecial>> or focusedmon>>
        if let Some(data) = line.strip_prefix("focusedmon>>") {
            let parts: Vec<&str> = data.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(id) = parts[1].parse::<i32>() {
                    return Some(WorkspaceEvent {
                        workspace_id: id,
                        workspace_name: String::new(),
                        monitor: parts[0].to_string(),
                    });
                }
            }
        }

        None
    }

    /// Receive the next workspace event
    pub async fn recv(&mut self) -> Option<WorkspaceEvent> {
        self.rx.recv().await
    }
}

/// Check if running under Hyprland
pub fn is_hyprland() -> bool {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}
