use crate::config::Config;
use crate::render::init_egl_display;
use crate::surface::WallpaperSurface;
use color_eyre::eyre::{Result, WrapErr};
use log::{debug, error, info, warn};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        LayerSurfaceConfigure,
    },
    shm::{Shm, ShmHandler},
};
use std::collections::HashMap;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
    Connection, QueueHandle,
};

extern crate khronos_egl as egl;

/// Main daemon state
pub struct Canviz {
    pub config: Config,
    pub registry_state: RegistryState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub shm: Shm,
    pub egl_display: egl::Display,
    pub surfaces: HashMap<String, WallpaperSurface>,
    pub exit: bool,
}

impl Canviz {
    pub fn new(
        config: Config,
        registry_state: RegistryState,
        output_state: OutputState,
        compositor_state: CompositorState,
        layer_shell: LayerShell,
        shm: Shm,
        egl_display: egl::Display,
    ) -> Self {
        Self {
            config,
            registry_state,
            output_state,
            compositor_state,
            layer_shell,
            shm,
            egl_display,
            surfaces: HashMap::new(),
            exit: false,
        }
    }

    /// Create a wallpaper surface for an output
    fn create_surface_for_output(
        &mut self,
        qh: &QueueHandle<Self>,
        output: &wl_output::WlOutput,
        output_name: String,
    ) -> Result<()> {
        info!("Creating wallpaper surface for output: {}", output_name);

        // Create the wayland surface
        let wl_surface = self.compositor_state.create_surface(qh);

        // Create layer surface on the background layer
        let layer_surface = self.layer_shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Background,
            Some("canviz"),
            Some(output),
        );

        // Configure layer surface
        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer_surface.set_exclusive_zone(-1); // Don't reserve space
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

        // Commit the surface to get the first configure event
        wl_surface.commit();

        // Get config for this monitor
        let monitor_config = self.config.get_monitor_config(&output_name);

        // Create our wallpaper surface wrapper
        let wallpaper_surface = WallpaperSurface::new(
            wl_surface,
            layer_surface,
            output.clone(),
            output_name.clone(),
            monitor_config,
            self.egl_display,
        )?;

        self.surfaces.insert(output_name, wallpaper_surface);

        Ok(())
    }
}

impl CompositorHandler for Canviz {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        // Find and update the surface that matches
        for (name, wallpaper_surface) in &mut self.surfaces {
            if wallpaper_surface.wl_surface() == surface {
                debug!("Scale factor changed for {}: {}", name, new_factor);
                if let Err(e) = wallpaper_surface.set_scale_factor(new_factor, qh) {
                    error!("Failed to update scale factor: {}", e);
                }
                break;
            }
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Handle transform changes if needed
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Find the surface and draw
        for (name, wallpaper_surface) in &mut self.surfaces {
            if wallpaper_surface.wl_surface() == surface {
                if let Err(e) = wallpaper_surface.draw(qh) {
                    error!("Failed to draw surface {}: {}", name, e);
                }
                break;
            }
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Canviz {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // Get output info
        let info = self.output_state.info(&output);
        let output_name = info
            .as_ref()
            .and_then(|i| i.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        info!("New output detected: {}", output_name);

        if let Err(e) = self.create_surface_for_output(qh, &output, output_name.clone()) {
            error!("Failed to create surface for {}: {}", output_name, e);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let info = self.output_state.info(&output);
        if let Some(info) = info {
            debug!("Output updated: {:?}", info.name);
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let info = self.output_state.info(&output);
        let output_name = info
            .as_ref()
            .and_then(|i| i.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        info!("Output removed: {}", output_name);
        self.surfaces.remove(&output_name);
    }
}

impl LayerShellHandler for Canviz {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        // Layer surface closed
        warn!("Layer surface closed");
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Find the surface that matches this layer surface
        for (name, wallpaper_surface) in &mut self.surfaces {
            if wallpaper_surface.layer_surface() == layer {
                debug!(
                    "Configure event for {}: {}x{}",
                    name, configure.new_size.0, configure.new_size.1
                );

                if let Err(e) = wallpaper_surface.configure(configure, qh) {
                    error!("Failed to configure surface {}: {}", name, e);
                }
                break;
            }
        }
    }
}

impl ShmHandler for Canviz {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Canviz {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

delegate_compositor!(Canviz);
delegate_output!(Canviz);
delegate_layer!(Canviz);
delegate_shm!(Canviz);
delegate_registry!(Canviz);

/// Main daemon entry point
pub fn run(config: Config, _foreground: bool) -> Result<()> {
    info!("Initializing Wayland connection");

    // Connect to Wayland
    let conn = Connection::connect_to_env()
        .wrap_err("Failed to connect to Wayland compositor")?;

    // Initialize EGL with Wayland display
    let egl_display = init_egl_display(&conn)
        .wrap_err("Failed to initialize EGL display")?;

    info!("EGL initialized successfully");

    // Initialize registry
    let (globals, mut event_queue) = registry_queue_init(&conn)
        .wrap_err("Failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    // Create state objects
    let compositor_state = CompositorState::bind(&globals, &qh)
        .wrap_err("Failed to bind compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .wrap_err("Failed to bind layer shell - is this a wlroots-based compositor?")?;
    let output_state = OutputState::new(&globals, &qh);
    let shm = Shm::bind(&globals, &qh)
        .wrap_err("Failed to bind shm")?;
    let registry_state = RegistryState::new(&globals);

    // Create main daemon state
    let mut canviz = Canviz::new(
        config,
        registry_state,
        output_state,
        compositor_state,
        layer_shell,
        shm,
        egl_display,
    );

    info!("Starting event loop");

    // Main event loop
    loop {
        if canviz.exit {
            info!("Exit requested, shutting down");
            break;
        }

        event_queue
            .blocking_dispatch(&mut canviz)
            .wrap_err("Wayland dispatch failed")?;
    }

    Ok(())
}
