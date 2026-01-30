#!/usr/bin/env bash
#
# Canviz Install Script
# Installs canviz wallpaper daemon for Hyprland/wlroots compositors
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Config
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/canviz}"

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

print_banner() {
    echo ""
    echo -e "${BLUE}"
    echo "   ____                  _     "
    echo "  / ___|__ _ _ ____   _(_)____"
    echo " | |   / _\` | '_ \\ \\ / / |_  /"
    echo " | |__| (_| | | | \\ V /| |/ / "
    echo "  \\____\\__,_|_| |_|\\_/ |_/___|"
    echo -e "${NC}"
    echo "  Hardware-accelerated wallpaper daemon"
    echo ""
}

# Detect distro
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO=$ID
    elif [ -f /etc/arch-release ]; then
        DISTRO="arch"
    elif [ -f /etc/debian_version ]; then
        DISTRO="debian"
    elif [ -f /etc/fedora-release ]; then
        DISTRO="fedora"
    else
        DISTRO="unknown"
    fi
    echo "$DISTRO"
}

# Check if command exists
has_cmd() { command -v "$1" &>/dev/null; }

# Check dependencies
check_deps() {
    info "Checking dependencies..."

    local missing=()

    # Check Rust
    if ! has_cmd cargo; then
        missing+=("rust/cargo")
    else
        success "Rust/Cargo found"
    fi

    # Check pkg-config
    if ! has_cmd pkg-config; then
        missing+=("pkg-config")
    else
        success "pkg-config found"
    fi

    # Check for wayland libs (via pkg-config if available)
    if has_cmd pkg-config; then
        if ! pkg-config --exists wayland-client 2>/dev/null; then
            missing+=("wayland-client")
        else
            success "wayland-client found"
        fi

        if ! pkg-config --exists egl 2>/dev/null; then
            missing+=("egl")
        else
            success "EGL found"
        fi
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        warn "Missing dependencies: ${missing[*]}"
        return 1
    fi

    return 0
}

# Print install commands for distro
print_install_cmd() {
    local distro=$(detect_distro)

    echo ""
    warn "Please install the required dependencies first."
    echo ""

    case "$distro" in
        arch|endeavouros|manjaro)
            echo "For Arch Linux:"
            echo -e "${GREEN}  sudo pacman -S --needed rust wayland wayland-protocols libglvnd mesa pkgconf${NC}"
            ;;
        fedora)
            echo "For Fedora:"
            echo -e "${GREEN}  sudo dnf install rust cargo wayland-devel wayland-protocols-devel mesa-libGL-devel mesa-libEGL-devel pkgconf${NC}"
            ;;
        ubuntu|debian|pop)
            echo "For Ubuntu/Debian:"
            echo -e "${GREEN}  sudo apt install rustc cargo libwayland-dev wayland-protocols libgl-dev libegl-dev pkg-config${NC}"
            ;;
        opensuse*)
            echo "For openSUSE:"
            echo -e "${GREEN}  sudo zypper install rust cargo wayland-devel wayland-protocols-devel Mesa-libGL-devel Mesa-libEGL-devel pkg-config${NC}"
            ;;
        *)
            echo "Install the following packages using your package manager:"
            echo "  - rust/cargo"
            echo "  - wayland development libraries"
            echo "  - wayland-protocols"
            echo "  - EGL/OpenGL development libraries"
            echo "  - pkg-config"
            ;;
    esac
    echo ""
}

# Build project
build() {
    info "Building canviz (release mode)..."

    if ! cargo build --release; then
        error "Build failed. Check the error messages above."
    fi

    success "Build complete"
}

# Install binaries
install_bins() {
    info "Installing to $INSTALL_DIR..."

    mkdir -p "$INSTALL_DIR"

    # Install main daemon
    if [ -f "target/release/canviz" ]; then
        cp target/release/canviz "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/canviz"
        success "Installed canviz"
    else
        error "canviz binary not found"
    fi

    # Install control tool if it exists
    if [ -f "target/release/canvizctl" ]; then
        cp target/release/canvizctl "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/canvizctl"
        success "Installed canvizctl"
    fi

    # Check if INSTALL_DIR is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add this to your ~/.bashrc or ~/.zshrc:"
        echo -e "${GREEN}  export PATH=\"\$PATH:$INSTALL_DIR\"${NC}"
        echo ""
    fi
}

# Get first monitor name
get_first_monitor() {
    if has_cmd hyprctl; then
        hyprctl monitors -j 2>/dev/null | grep -oP '"name":\s*"\K[^"]+' | head -1
    elif has_cmd wlr-randr; then
        wlr-randr 2>/dev/null | grep -oP '^\S+' | head -1
    else
        echo "eDP-1"  # Common default
    fi
}

# Create default config
create_config() {
    info "Setting up configuration..."

    mkdir -p "$CONFIG_DIR"

    if [ -f "$CONFIG_DIR/config.toml" ]; then
        warn "Config already exists at $CONFIG_DIR/config.toml"
        read -p "Overwrite? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            info "Keeping existing config"
            return
        fi
    fi

    # Try to detect monitor name
    local monitor=$(get_first_monitor)

    # Find a wallpaper
    local wallpaper="~/Pictures/wallpaper.jpg"
    for dir in "$HOME/Pictures" "$HOME/Wallpapers" "/usr/share/backgrounds"; do
        if [ -d "$dir" ]; then
            local found=$(find "$dir" -maxdepth 2 -type f \( -name "*.jpg" -o -name "*.png" -o -name "*.webp" \) 2>/dev/null | head -1)
            if [ -n "$found" ]; then
                wallpaper="${found/#$HOME/\~}"
                break
            fi
        fi
    done

    cat > "$CONFIG_DIR/config.toml" << EOF
# Canviz Configuration
# See: https://github.com/javanhut/Canviz

# Global defaults for all monitors
[default]
transition = "fade"
transition_time = 300
mode = "cover"

# Per-monitor configuration
# Find your monitor names with: hyprctl monitors
[$monitor]
path = "$wallpaper"

# Example: Slideshow on external monitor
# [monitors.DP-1]
# path = "~/Pictures/Wallpapers"
# duration = "30m"
# sorting = "random"
# recursive = true
EOF

    success "Created config at $CONFIG_DIR/config.toml"
    info "Detected monitor: $monitor"
    info "Edit the config to customize your wallpapers"
}

# Show post-install instructions
post_install() {
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN} Installation complete!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Quick start:"
    echo ""
    echo "  1. Edit your config:"
    echo -e "     ${BLUE}$CONFIG_DIR/config.toml${NC}"
    echo ""
    echo "  2. Test canviz:"
    echo -e "     ${GREEN}canviz -v -f${NC}"
    echo ""
    echo "  3. Autostart with Hyprland - add to hyprland.conf:"
    echo -e "     ${GREEN}exec-once = canviz${NC}"
    echo ""
    echo "  4. Find your monitor names:"
    echo -e "     ${GREEN}hyprctl monitors${NC}"
    echo ""
    echo "Documentation: https://github.com/javanhut/Canviz"
    echo ""
}

# Main
main() {
    print_banner

    # Check we're in the right directory
    if [ ! -f "Cargo.toml" ]; then
        error "Run this script from the canviz source directory"
    fi

    # Parse args
    SKIP_DEPS=false
    for arg in "$@"; do
        case $arg in
            --skip-deps)
                SKIP_DEPS=true
                ;;
            --help|-h)
                echo "Usage: ./install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --skip-deps    Skip dependency check"
                echo "  --help         Show this help"
                echo ""
                echo "Environment variables:"
                echo "  INSTALL_DIR    Binary install location (default: ~/.local/bin)"
                echo "  CONFIG_DIR     Config location (default: ~/.config/canviz)"
                exit 0
                ;;
        esac
    done

    # Check deps
    if [ "$SKIP_DEPS" = false ]; then
        if ! check_deps; then
            print_install_cmd
            echo "After installing dependencies, run this script again."
            echo "Or run with --skip-deps to skip this check."
            exit 1
        fi
    fi

    echo ""

    # Build
    build

    echo ""

    # Install
    install_bins

    echo ""

    # Config
    create_config

    # Done
    post_install
}

main "$@"
