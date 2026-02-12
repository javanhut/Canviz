use crate::config::{BackgroundMode, MonitorConfig, TransitionType};
use crate::daemon::Canviz;
use crate::render::{EglContext, Renderer};
use color_eyre::eyre::{Result, WrapErr};
use log::{debug, error, info, warn};
use smithay_client_toolkit::shell::wlr_layer::{LayerSurface, LayerSurfaceConfigure};
use std::path::PathBuf;
use std::time::Instant;
use wayland_client::{protocol::wl_output::WlOutput, protocol::wl_surface::WlSurface, QueueHandle};

extern crate khronos_egl as egl;

/// Represents a wallpaper surface for a single output/monitor
pub struct WallpaperSurface {
    wl_surface: WlSurface,
    layer_surface: LayerSurface,
    output: WlOutput,
    output_name: String,
    config: MonitorConfig,
    egl_display: egl::Display,
    egl_context: Option<EglContext>,
    renderer: Option<Renderer>,
    width: u32,
    height: u32,
    scale_factor: i32,
    configured: bool,
    last_frame_time: Option<Instant>,
    current_wallpaper_path: Option<PathBuf>,
}

impl WallpaperSurface {
    pub fn new(
        wl_surface: WlSurface,
        layer_surface: LayerSurface,
        output: WlOutput,
        output_name: String,
        config: MonitorConfig,
        egl_display: egl::Display,
    ) -> Result<Self> {
        Ok(Self {
            wl_surface,
            layer_surface,
            output,
            output_name,
            config,
            egl_display,
            egl_context: None,
            renderer: None,
            width: 0,
            height: 0,
            scale_factor: 1,
            configured: false,
            last_frame_time: None,
            current_wallpaper_path: None,
        })
    }

    pub fn wl_surface(&self) -> &WlSurface {
        &self.wl_surface
    }

    pub fn layer_surface(&self) -> &LayerSurface {
        &self.layer_surface
    }

    #[allow(dead_code)]
    pub fn output(&self) -> &WlOutput {
        &self.output
    }

    pub fn output_name(&self) -> &str {
        &self.output_name
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &MonitorConfig {
        &self.config
    }

    /// Handle configure event from the compositor
    pub fn configure(
        &mut self,
        configure: LayerSurfaceConfigure,
        qh: &QueueHandle<Canviz>,
    ) -> Result<()> {
        let (width, height) = configure.new_size;

        // Use suggested size or fall back to some defaults
        let width = if width == 0 { 1920 } else { width };
        let height = if height == 0 { 1080 } else { height };

        info!(
            "Configuring surface {} with size {}x{}",
            self.output_name, width, height
        );

        let size_changed = self.width != width || self.height != height;
        self.width = width;
        self.height = height;
        self.configured = true;

        // Note: Don't call set_size() here - the compositor already told us the size
        // in the configure event. Calling set_size() would trigger another configure.

        // Initialize or resize EGL context
        let first_configure = self.egl_context.is_none();
        if first_configure {
            self.init_rendering()?;
            // Load initial wallpaper only on first configure
            self.load_initial_wallpaper();
        } else if size_changed {
            self.resize_rendering()?;
        }

        // Do the first draw immediately - this will commit
        // Don't request a separate frame callback here, as that + commit triggers infinite configures
        self.draw_frame(qh)?;

        Ok(())
    }

    /// Initialize EGL context and renderer
    fn init_rendering(&mut self) -> Result<()> {
        info!("Initializing rendering for {}", self.output_name);

        // Calculate buffer size with scale factor
        let buffer_width = self.width * self.scale_factor as u32;
        let buffer_height = self.height * self.scale_factor as u32;

        // Create EGL context
        let egl_context = EglContext::new(
            self.egl_display,
            &self.wl_surface,
            buffer_width,
            buffer_height,
        )
        .wrap_err_with(|| format!("Failed to create EGL context for {}", self.output_name))?;

        // Create renderer
        let transition_type = self.config.transition.unwrap_or(TransitionType::Fade);
        let transition_time = self.config.transition_time.unwrap_or(300);
        let background_mode = self.config.mode.unwrap_or(BackgroundMode::Cover);

        let mut renderer = Renderer::new(transition_type, transition_time, background_mode)
            .wrap_err("Failed to create renderer")?;

        renderer.set_viewport(buffer_width, buffer_height);

        self.egl_context = Some(egl_context);
        self.renderer = Some(renderer);

        info!("Rendering initialized for {} ({}x{})", self.output_name, buffer_width, buffer_height);

        Ok(())
    }

    /// Resize the rendering context
    fn resize_rendering(&mut self) -> Result<()> {
        let buffer_width = self.width * self.scale_factor as u32;
        let buffer_height = self.height * self.scale_factor as u32;

        if let Some(ref mut ctx) = self.egl_context {
            ctx.resize(buffer_width, buffer_height)?;
        }

        if let Some(ref mut renderer) = self.renderer {
            renderer.set_viewport(buffer_width, buffer_height);
        }

        Ok(())
    }

    /// Load initial wallpaper from config
    fn load_initial_wallpaper(&mut self) {
        let path = &self.config.path;

        if path.as_os_str().is_empty() {
            warn!("No wallpaper path configured for {}", self.output_name);
            // Set a default dark color
            if let Some(ref mut renderer) = self.renderer {
                if let Err(e) = renderer.set_solid_color(30, 30, 40) {
                    error!("Failed to set solid color: {}", e);
                }
            }
            return;
        }

        // Expand ~ to home directory
        let expanded_path = if path.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                home.join(path.strip_prefix("~").unwrap_or(path))
            } else {
                path.clone()
            }
        } else {
            path.clone()
        };

        if expanded_path.is_file() {
            if let Err(e) = self.load_wallpaper(&expanded_path) {
                error!("Failed to load wallpaper {:?}: {}", expanded_path, e);
                // Fallback to solid color
                if let Some(ref mut renderer) = self.renderer {
                    let _ = renderer.set_solid_color(30, 30, 40);
                }
            }
        } else if expanded_path.is_dir() {
            // For directories, pick the first image (slideshow logic will come later)
            if let Ok(entries) = std::fs::read_dir(&expanded_path) {
                let extensions = ["jpg", "jpeg", "png", "bmp", "gif", "webp"];
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if let Some(ext) = entry_path.extension() {
                        if extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                            if let Err(e) = self.load_wallpaper(&entry_path) {
                                error!("Failed to load wallpaper {:?}: {}", entry_path, e);
                            } else {
                                return;
                            }
                        }
                    }
                }
            }
            warn!("No images found in directory {:?}", expanded_path);
            if let Some(ref mut renderer) = self.renderer {
                let _ = renderer.set_solid_color(30, 30, 40);
            }
        } else {
            warn!("Wallpaper path does not exist: {:?}", expanded_path);
            if let Some(ref mut renderer) = self.renderer {
                let _ = renderer.set_solid_color(30, 30, 40);
            }
        }
    }

    /// Load a wallpaper from a file path
    pub fn load_wallpaper(&mut self, path: &std::path::Path) -> Result<()> {
        if let Some(ref mut ctx) = self.egl_context {
            ctx.make_current()?;
        }

        if let Some(ref mut renderer) = self.renderer {
            renderer.load_wallpaper_from_file(path)?;
            self.current_wallpaper_path = Some(path.to_path_buf());
            info!("Loaded wallpaper: {:?}", path);
        }

        Ok(())
    }

    /// Set scale factor for HiDPI support
    pub fn set_scale_factor(&mut self, factor: i32, qh: &QueueHandle<Canviz>) -> Result<()> {
        if factor != self.scale_factor {
            info!(
                "Scale factor changed for {}: {} -> {}",
                self.output_name, self.scale_factor, factor
            );
            self.scale_factor = factor;
            self.wl_surface.set_buffer_scale(factor);

            // Resize rendering
            if self.configured {
                self.resize_rendering()?;
            }

            // Request redraw
            self.wl_surface.frame(qh, self.wl_surface.clone());
            self.wl_surface.commit();
        }
        Ok(())
    }

    /// Internal method to render a frame without checking configured state
    fn draw_frame(&mut self, qh: &QueueHandle<Canviz>) -> Result<()> {
        // Calculate delta time for transitions
        let now = Instant::now();
        let delta_ms = if let Some(last) = self.last_frame_time {
            now.duration_since(last).as_millis() as u32
        } else {
            16 // Assume 60fps for first frame
        };
        self.last_frame_time = Some(now);

        // Make EGL context current
        if let Some(ref ctx) = self.egl_context {
            ctx.make_current()?;
        } else {
            return Ok(());
        }

        // Update and render
        let needs_redraw = if let Some(ref mut renderer) = self.renderer {
            let animating = renderer.update(delta_ms);
            renderer.render();
            animating
        } else {
            false
        };

        // Swap buffers
        if let Some(ref ctx) = self.egl_context {
            ctx.swap_buffers()?;
        }

        // Mark surface as damaged
        self.wl_surface.damage_buffer(
            0,
            0,
            (self.width * self.scale_factor as u32) as i32,
            (self.height * self.scale_factor as u32) as i32,
        );
        self.wl_surface.commit();

        // Request next frame if still animating
        if needs_redraw {
            self.wl_surface.frame(qh, self.wl_surface.clone());
        }

        Ok(())
    }

    /// Draw the wallpaper (called from frame callback)
    pub fn draw(&mut self, qh: &QueueHandle<Canviz>) -> Result<()> {
        if !self.configured {
            debug!("Surface {} not yet configured, skipping draw", self.output_name);
            return Ok(());
        }

        self.draw_frame(qh)
    }

    /// Check if surface is configured and ready
    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// Get current wallpaper path
    #[allow(dead_code)]
    pub fn current_wallpaper(&self) -> Option<&PathBuf> {
        self.current_wallpaper_path.as_ref()
    }
}

impl Drop for WallpaperSurface {
    fn drop(&mut self) {
        info!("Destroying wallpaper surface for {}", self.output_name);
    }
}
