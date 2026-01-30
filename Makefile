# Canviz Makefile
# Hardware-accelerated wallpaper daemon for Hyprland

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
CONFIGDIR ?= $(HOME)/.config/canviz

# Binary names
DAEMON = canviz
CTL = canvizctl

# Build targets
.PHONY: all build release debug clean install uninstall config help

all: release

# Build release binary (optimized)
release:
	@echo "Building release..."
	cargo build --release
	@echo "Done. Binaries in target/release/"

# Build debug binary
debug:
	@echo "Building debug..."
	cargo build
	@echo "Done. Binaries in target/debug/"

# Run cargo check
check:
	cargo check

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean
	@echo "Cleaned build artifacts"

# Install binaries and config
install: release
	@echo "Installing to $(BINDIR)..."
	@mkdir -p $(BINDIR)
	@cp target/release/$(DAEMON) $(BINDIR)/
	@chmod +x $(BINDIR)/$(DAEMON)
	@echo "Installed $(DAEMON)"
	@if [ -f target/release/$(CTL) ]; then \
		cp target/release/$(CTL) $(BINDIR)/; \
		chmod +x $(BINDIR)/$(CTL); \
		echo "Installed $(CTL)"; \
	fi
	@echo ""
	@echo "Run 'make config' to create default configuration"
	@echo "Or run 'make install-config' to install config now"

# Install config file (won't overwrite existing)
config:
	@mkdir -p $(CONFIGDIR)
	@if [ -f $(CONFIGDIR)/config.toml ]; then \
		echo "Config already exists at $(CONFIGDIR)/config.toml"; \
		echo "Remove it first or edit manually"; \
	else \
		cp config.example.toml $(CONFIGDIR)/config.toml; \
		echo "Created $(CONFIGDIR)/config.toml"; \
		echo "Edit this file to set your wallpapers"; \
	fi

# Install everything including config
install-all: install config
	@echo ""
	@echo "Installation complete!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Edit $(CONFIGDIR)/config.toml"
	@echo "  2. Run: $(DAEMON) -v -f"
	@echo "  3. Add to hyprland.conf: exec-once = $(DAEMON)"

# Uninstall binaries
uninstall:
	@echo "Removing binaries..."
	@rm -f $(BINDIR)/$(DAEMON)
	@rm -f $(BINDIR)/$(CTL)
	@echo "Removed $(DAEMON) and $(CTL) from $(BINDIR)"
	@echo ""
	@echo "Config at $(CONFIGDIR) was NOT removed"
	@echo "Run 'make uninstall-all' to remove config too"

# Uninstall everything including config
uninstall-all: uninstall
	@echo "Removing config..."
	@rm -rf $(CONFIGDIR)
	@echo "Removed $(CONFIGDIR)"

# Run in foreground (for testing)
run: release
	./target/release/$(DAEMON) -v -f

# Run debug build
run-debug: debug
	RUST_BACKTRACE=1 ./target/debug/$(DAEMON) -v -f

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy

# Show help
help:
	@echo "Canviz Makefile"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build targets:"
	@echo "  all, release   Build optimized release binary (default)"
	@echo "  debug          Build debug binary"
	@echo "  check          Run cargo check"
	@echo "  test           Run tests"
	@echo "  clean          Remove build artifacts"
	@echo ""
	@echo "Install targets:"
	@echo "  install        Install binaries to $(BINDIR)"
	@echo "  config         Create config file (won't overwrite)"
	@echo "  install-all    Install binaries and config"
	@echo "  uninstall      Remove binaries"
	@echo "  uninstall-all  Remove binaries and config"
	@echo ""
	@echo "Development:"
	@echo "  run            Build and run (foreground, verbose)"
	@echo "  run-debug      Build and run debug build"
	@echo "  fmt            Format code"
	@echo "  lint           Run clippy"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=$(PREFIX)"
	@echo "  BINDIR=$(BINDIR)"
	@echo "  CONFIGDIR=$(CONFIGDIR)"
	@echo ""
	@echo "Example: make install PREFIX=/usr/local"
