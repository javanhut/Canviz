use super::gl;
use color_eyre::eyre::{eyre, Result, WrapErr};
use log::{debug, error, info};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};
use wayland_egl::WlEglSurface;

extern crate khronos_egl as egl;
use egl::API as egl_api;

/// Initialize EGL display for Wayland
pub fn init_egl_display(conn: &Connection) -> Result<egl::Display> {
    // Bind OpenGL ES API
    egl_api
        .bind_api(egl::OPENGL_ES_API)
        .wrap_err("Failed to bind OpenGL ES API")?;

    // Get native Wayland display
    let wayland_display = conn.backend().display_ptr() as *mut std::ffi::c_void;

    // Get EGL display using the Wayland display
    let display = unsafe {
        egl_api.get_display(wayland_display as egl::NativeDisplayType)
            .ok_or_else(|| eyre!("Failed to get EGL display"))?
    };

    // Initialize EGL
    egl_api
        .initialize(display)
        .wrap_err("Failed to initialize EGL display")?;

    // Log EGL info
    if let Ok(vendor) = egl_api.query_string(Some(display), egl::VENDOR) {
        info!("EGL Vendor: {}", vendor.to_string_lossy());
    }
    if let Ok(version) = egl_api.query_string(Some(display), egl::VERSION) {
        info!("EGL Version: {}", version.to_string_lossy());
    }

    Ok(display)
}

/// EGL context for OpenGL ES rendering
pub struct EglContext {
    display: egl::Display,
    context: egl::Context,
    surface: egl::Surface,
    wl_egl_surface: WlEglSurface,
    _config: egl::Config,
}

impl EglContext {
    /// Create a new EGL context for the given Wayland surface
    pub fn new(
        egl_display: egl::Display,
        wl_surface: &WlSurface,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        info!("Creating EGL context ({}x{})", width, height);

        // Choose EGL config
        let config_attribs = [
            egl::RED_SIZE, 8,
            egl::GREEN_SIZE, 8,
            egl::BLUE_SIZE, 8,
            egl::ALPHA_SIZE, 8,
            egl::SURFACE_TYPE, egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
            egl::NONE,
        ];

        let config = egl_api
            .choose_first_config(egl_display, &config_attribs)
            .wrap_err("Failed to choose EGL config")?
            .ok_or_else(|| eyre!("No suitable EGL config found"))?;

        debug!("EGL config chosen successfully");

        // Create EGL context
        let context_attribs = [
            egl::CONTEXT_CLIENT_VERSION, 2,
            egl::NONE,
        ];

        let context = egl_api
            .create_context(egl_display, config, None, &context_attribs)
            .wrap_err("Failed to create EGL context")?;

        debug!("EGL context created");

        // Create Wayland EGL surface
        let wl_egl_surface = WlEglSurface::new(wl_surface.id(), width as i32, height as i32)
            .wrap_err("Failed to create Wayland EGL surface")?;

        debug!("Wayland EGL surface created");

        // Create EGL window surface
        let surface_attribs = [egl::NONE];
        let surface = unsafe {
            egl_api
                .create_window_surface(
                    egl_display,
                    config,
                    wl_egl_surface.ptr() as egl::NativeWindowType,
                    Some(&surface_attribs),
                )
                .wrap_err("Failed to create EGL window surface")?
        };

        debug!("EGL window surface created");

        // Make context current
        egl_api
            .make_current(egl_display, Some(surface), Some(surface), Some(context))
            .wrap_err("Failed to make EGL context current")?;

        debug!("EGL context made current");

        // Load OpenGL ES functions
        gl::load_with(|name| {
            egl_api
                .get_proc_address(name)
                .map(|p| p as *const std::ffi::c_void)
                .unwrap_or(std::ptr::null())
        });

        // Log OpenGL info
        unsafe {
            let version = gl::GetString(gl::VERSION);
            let vendor = gl::GetString(gl::VENDOR);
            let renderer = gl::GetString(gl::RENDERER);

            if !version.is_null() {
                info!(
                    "OpenGL ES: {} / {} / {}",
                    std::ffi::CStr::from_ptr(version as *const i8).to_string_lossy(),
                    std::ffi::CStr::from_ptr(vendor as *const i8).to_string_lossy(),
                    std::ffi::CStr::from_ptr(renderer as *const i8).to_string_lossy()
                );
            }
        }

        info!("EGL context created successfully for surface");

        Ok(Self {
            display: egl_display,
            context,
            surface,
            wl_egl_surface,
            _config: config,
        })
    }

    /// Make this context current
    pub fn make_current(&self) -> Result<()> {
        egl_api
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .wrap_err("Failed to make EGL context current")
    }

    /// Swap buffers (present the rendered frame)
    pub fn swap_buffers(&self) -> Result<()> {
        egl_api
            .swap_buffers(self.display, self.surface)
            .wrap_err("Failed to swap EGL buffers")
    }

    /// Resize the EGL surface
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        debug!("Resizing EGL surface to {}x{}", width, height);
        self.wl_egl_surface.resize(width as i32, height as i32, 0, 0);
        Ok(())
    }
}

impl Drop for EglContext {
    fn drop(&mut self) {
        info!("Destroying EGL context");

        // Make no context current
        let _ = egl_api.make_current(self.display, None, None, None);

        // Destroy surface and context
        let _ = egl_api.destroy_surface(self.display, self.surface);
        let _ = egl_api.destroy_context(self.display, self.context);
    }
}
