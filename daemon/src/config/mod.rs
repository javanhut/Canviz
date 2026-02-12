use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Main configuration structure
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default settings applied to all monitors/workspaces unless overridden
    pub default: DefaultConfig,
    /// Per-monitor wallpaper configuration
    #[serde(default)]
    pub monitors: HashMap<String, MonitorConfig>,
    /// Per-workspace wallpaper configuration (primary feature)
    #[serde(default)]
    pub workspaces: WorkspaceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default: DefaultConfig::default(),
            monitors: HashMap::new(),
            workspaces: WorkspaceConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            log::warn!("Config file not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get wallpaper path for a specific workspace on a monitor
    pub fn get_wallpaper_for_workspace(&self, monitor: &str, workspace: i32) -> Option<PathBuf> {
        // First check workspace-specific config
        if self.workspaces.enabled {
            if let Some(path) = self.workspaces.wallpapers.get(&workspace) {
                return Some(expand_path(path));
            }
        }

        // Fall back to monitor-specific config
        if let Some(mon_config) = self.monitors.get(monitor) {
            return Some(expand_path(&mon_config.path));
        }

        // Fall back to default
        self.default.path.as_ref().map(|p| expand_path(p))
    }

    /// Get the monitor config, falling back to defaults
    pub fn get_monitor_config(&self, monitor: &str) -> MonitorConfig {
        self.monitors
            .get(monitor)
            .cloned()
            .unwrap_or_else(|| MonitorConfig::from_default(&self.default))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DefaultConfig {
    /// Default wallpaper path (file or directory)
    pub path: Option<PathBuf>,
    /// Transition type
    pub transition: TransitionType,
    /// Transition duration in milliseconds
    pub transition_time: u32,
    /// Background mode
    pub mode: BackgroundMode,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            path: None,
            transition: TransitionType::Fade,
            transition_time: 300,
            mode: BackgroundMode::Cover,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MonitorConfig {
    /// Wallpaper path (file or directory)
    pub path: PathBuf,
    /// Slideshow duration (if path is a directory)
    #[serde(default, with = "humantime_serde")]
    pub duration: Option<Duration>,
    /// Sorting method for slideshow
    pub sorting: SortingMethod,
    /// Search subdirectories
    pub recursive: bool,
    /// Background mode override
    pub mode: Option<BackgroundMode>,
    /// Transition type override
    pub transition: Option<TransitionType>,
    /// Transition time override
    pub transition_time: Option<u32>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            duration: None,
            sorting: SortingMethod::Random,
            recursive: true,
            mode: None,
            transition: None,
            transition_time: None,
        }
    }
}

impl MonitorConfig {
    pub fn from_default(default: &DefaultConfig) -> Self {
        Self {
            path: default.path.clone().unwrap_or_default(),
            duration: None,
            sorting: SortingMethod::Random,
            recursive: true,
            mode: Some(default.mode),
            transition: Some(default.transition),
            transition_time: Some(default.transition_time),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    /// Enable per-workspace wallpapers
    pub enabled: bool,
    /// Workspace number -> wallpaper path mapping
    #[serde(flatten)]
    pub wallpapers: HashMap<i32, PathBuf>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            wallpapers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransitionType {
    /// No transition, instant switch
    None,
    /// Simple fade/crossfade
    #[default]
    Fade,
    /// Slide from a direction
    Slide,
    /// Wipe effect
    Wipe,
    /// Crossfade with easing
    Crossfade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundMode {
    /// Scale to cover entire screen, may crop
    #[default]
    Cover,
    /// Scale to fit within screen, may letterbox
    Contain,
    /// Stretch to fill, may distort
    Fill,
    /// Tile the image
    Tile,
    /// Center without scaling
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortingMethod {
    /// Random order
    #[default]
    Random,
    /// Alphabetical ascending
    Ascending,
    /// Alphabetical descending
    Descending,
}

/// Expand ~ to home directory
fn expand_path(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.workspaces.enabled);
        assert_eq!(config.default.transition, TransitionType::Fade);
        assert_eq!(config.default.mode, BackgroundMode::Cover);
    }

    #[test]
    fn test_expand_path() {
        let path = Path::new("~/Pictures/test.jpg");
        let expanded = expand_path(path);
        assert!(!expanded.starts_with("~"));
    }
}
