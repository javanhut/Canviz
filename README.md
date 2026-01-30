# Canviz

A modern, hardware-accelerated wallpaper daemon for Hyprland and wlroots-based Wayland compositors.

## Features

- **Hardware-accelerated rendering** via OpenGL ES
- **Smooth transitions** between wallpapers (fade, slide, wipe)
- **Per-monitor wallpapers** with individual settings
- **Per-workspace wallpapers** (Hyprland-specific, coming soon)
- **Slideshow support** with configurable intervals
- **Simple TOML configuration**
- **Hot-reload** - config changes apply automatically

## Quick Start

### Using Make (Recommended)

```bash
make install        # Build and install binaries
make config         # Create default config
```

Or do everything at once:
```bash
make install-all
```

### Using Install Script

```bash
./install.sh
```

This will:
1. Check and install system dependencies
2. Build the project
3. Install binaries to `~/.local/bin`
4. Create a default config at `~/.config/canviz/config.toml`

### Manual Installation

#### 1. Install Dependencies

**Arch Linux:**
```bash
sudo pacman -S --needed rust wayland wayland-protocols libglvnd mesa
```

**Fedora:**
```bash
sudo dnf install rust cargo wayland-devel wayland-protocols-devel mesa-libGL-devel mesa-libEGL-devel
```

**Ubuntu/Debian:**
```bash
sudo apt install rustc cargo libwayland-dev wayland-protocols libgl-dev libegl-dev
```

#### 2. Build

```bash
cargo build --release
```

#### 3. Install

```bash
# Copy binaries
cp target/release/canviz ~/.local/bin/
cp target/release/canvizctl ~/.local/bin/

# Create config directory
mkdir -p ~/.config/canviz
```

#### 4. Create Configuration

Create `~/.config/canviz/config.toml`:

```toml
[default]
transition = "fade"
transition_time = 300
mode = "cover"

[monitors.eDP-1]
path = "~/Pictures/wallpaper.jpg"
```

**Find your monitor names:**
```bash
hyprctl monitors | grep "Monitor"
# or
wlr-randr
```

#### 5. Run

```bash
canviz -f  # Foreground mode for testing
```

### Autostart with Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```conf
exec-once = canviz
```

**Important:** Remove or disable other wallpaper daemons first:
```bash
# Comment out or remove these from hyprland.conf:
# exec-once = hyprpaper
# exec-once = swaybg
# exec-once = wpaperd
```

---

## Configuration

Config location: `~/.config/canviz/config.toml`

### Minimal Example

```toml
[monitors.eDP-1]
path = "~/Pictures/wallpaper.jpg"
```

### Full Example

```toml
[default]
transition = "fade"
transition_time = 500
mode = "cover"

# Laptop display - static wallpaper
[monitors.eDP-1]
path = "~/Pictures/laptop-wallpaper.jpg"

# External monitor - slideshow
[monitors.DP-1]
path = "~/Pictures/Wallpapers"
duration = "15m"
sorting = "random"
recursive = true
transition = "slide_left"
transition_time = 800

# Portrait monitor
[monitors.HDMI-A-1]
path = "~/Pictures/portrait-wallpaper.jpg"
mode = "contain"
```

### Configuration Reference

#### `[default]` - Global Defaults

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `transition` | string | `"fade"` | Transition effect |
| `transition_time` | integer | `300` | Transition duration (ms) |
| `mode` | string | `"cover"` | Image scaling mode |

#### `[monitors.<name>]` - Per-Monitor Settings

| Option | Type | Description |
|--------|------|-------------|
| `path` | string | **Required.** Path to image or directory |
| `transition` | string | Override transition effect |
| `transition_time` | integer | Override transition duration |
| `mode` | string | Override scaling mode |
| `duration` | string | Slideshow interval (e.g., `"30m"`, `"1h"`) |
| `sorting` | string | Slideshow order: `random`, `ascending`, `descending` |
| `recursive` | bool | Search subdirectories for images |

### Scaling Modes

| Mode | Description |
|------|-------------|
| `cover` | Scale to fill, crop excess (default) |
| `contain` | Scale to fit, letterbox if needed |
| `fill` | Stretch to fill (may distort) |
| `tile` | Repeat as tiles |
| `center` | Center without scaling |

### Transitions

| Type | Description |
|------|-------------|
| `fade` | Crossfade (default) |
| `slide_left` | Slide from right |
| `slide_right` | Slide from left |
| `slide_up` | Slide from bottom |
| `slide_down` | Slide from top |
| `wipe` | Wipe transition |
| `none` | Instant switch |

### Slideshow Setup

Point `path` to a directory and set `duration`:

```toml
[monitors.DP-1]
path = "~/Pictures/Wallpapers"
duration = "30m"          # Change every 30 minutes
sorting = "random"        # Random order
recursive = true          # Include subdirectories
```

Duration formats: `30s`, `5m`, `1h`, `2h30m`

---

## CLI Usage

```bash
canviz [OPTIONS]

Options:
  -c, --config <PATH>    Config file path [default: ~/.config/canviz/config.toml]
  -f, --foreground       Run in foreground (don't daemonize)
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
  -V, --version          Print version
```

**Examples:**
```bash
# Test with verbose output
canviz -v -f

# Use custom config
canviz -c ~/my-config.toml -f

# Run as daemon
canviz
```

---

## Supported Formats

- JPEG (`.jpg`, `.jpeg`)
- PNG (`.png`)
- WebP (`.webp`)
- BMP (`.bmp`)
- GIF (`.gif`) - static only

---

## Troubleshooting

### Wallpaper not showing

1. **Verify monitor name:**
   ```bash
   hyprctl monitors
   ```

2. **Verify image path:**
   ```bash
   ls -la ~/Pictures/wallpaper.jpg
   ```

3. **Check logs:**
   ```bash
   canviz -v -f
   ```

### Black screen

Kill competing wallpaper daemons:
```bash
pkill hyprpaper; pkill swaybg; pkill wpaperd
```

### Config not reloading

Save the config file - Canviz watches for changes automatically.

### OpenGL errors

Check your GPU driver:
```bash
glxinfo | grep "OpenGL renderer"
eglinfo
```

---

## Building from Source

### Requirements

- Rust 1.70+
- System libraries: wayland, wayland-protocols, EGL, OpenGL ES

### Build Commands

Using Make:
```bash
make              # Build release
make debug        # Build debug
make test         # Run tests
make clean        # Clean artifacts
make help         # Show all targets
```

Using Cargo directly:
```bash
cargo build --release   # Release build
cargo build             # Debug build
cargo test              # Run tests
cargo check             # Check without building
```

Binaries are in `target/release/`:
- `canviz` - Wallpaper daemon
- `canvizctl` - Control tool (WIP)

---

## Uninstall

```bash
make uninstall      # Remove binaries only
make uninstall-all  # Remove binaries and config
```

Or manually:
```bash
rm ~/.local/bin/canviz
rm ~/.local/bin/canvizctl
rm -rf ~/.config/canviz
```

---

## Architecture

```
canviz/
├── daemon/              # Main daemon
│   └── src/
│       ├── main.rs      # Entry point
│       ├── daemon.rs    # Wayland event loop
│       ├── config/      # Config parsing
│       ├── surface/     # Monitor surfaces
│       ├── render/      # EGL/OpenGL
│       ├── image/       # Image loading
│       ├── ipc/         # Unix socket IPC
│       └── hyprland/    # Hyprland integration
└── ctl/                 # Control CLI
```

---

## License

GPL-3.0

## Author

[javanhut](https://github.com/javanhut)
