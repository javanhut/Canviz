use super::gl;
use crate::config::{BackgroundMode, TransitionType};
use color_eyre::eyre::{eyre, Result, WrapErr};
use log::{debug, error, info};
use std::ffi::CString;
use std::ptr;

const VERTEX_SHADER_SRC: &str = include_str!("shaders/vertex.glsl");
const FRAGMENT_SHADER_SRC: &str = include_str!("shaders/fragment.glsl");

/// Compiled shader program
pub struct ShaderProgram {
    pub program: u32,
    pub a_position: i32,
    pub a_texcoord: i32,
    pub u_texture: i32,
    pub u_texture_prev: i32,
    pub u_progress: i32,
    pub u_transition_type: i32,
}

impl ShaderProgram {
    pub fn new() -> Result<Self> {
        unsafe {
            // Compile vertex shader
            let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
            let vertex_src = CString::new(VERTEX_SHADER_SRC).unwrap();
            gl::ShaderSource(vertex_shader, 1, &vertex_src.as_ptr(), ptr::null());
            gl::CompileShader(vertex_shader);
            Self::check_shader_compile(vertex_shader, "vertex")?;

            // Compile fragment shader
            let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            let fragment_src = CString::new(FRAGMENT_SHADER_SRC).unwrap();
            gl::ShaderSource(fragment_shader, 1, &fragment_src.as_ptr(), ptr::null());
            gl::CompileShader(fragment_shader);
            Self::check_shader_compile(fragment_shader, "fragment")?;

            // Link program
            let program = gl::CreateProgram();
            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);
            gl::LinkProgram(program);
            Self::check_program_link(program)?;

            // Clean up shaders (they're linked now)
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            // Get attribute locations
            let pos_name = CString::new("a_position").unwrap();
            let tex_name = CString::new("a_texcoord").unwrap();
            let a_position = gl::GetAttribLocation(program, pos_name.as_ptr());
            let a_texcoord = gl::GetAttribLocation(program, tex_name.as_ptr());

            // Get uniform locations
            let u_tex_name = CString::new("u_texture").unwrap();
            let u_tex_prev_name = CString::new("u_texture_prev").unwrap();
            let u_prog_name = CString::new("u_progress").unwrap();
            let u_trans_name = CString::new("u_transition_type").unwrap();

            let u_texture = gl::GetUniformLocation(program, u_tex_name.as_ptr());
            let u_texture_prev = gl::GetUniformLocation(program, u_tex_prev_name.as_ptr());
            let u_progress = gl::GetUniformLocation(program, u_prog_name.as_ptr());
            let u_transition_type = gl::GetUniformLocation(program, u_trans_name.as_ptr());

            info!("Shader program compiled successfully");

            Ok(Self {
                program,
                a_position,
                a_texcoord,
                u_texture,
                u_texture_prev,
                u_progress,
                u_transition_type,
            })
        }
    }

    unsafe fn check_shader_compile(shader: u32, name: &str) -> Result<()> {
        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buffer = vec![0u8; len as usize];
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut i8);
            let error = String::from_utf8_lossy(&buffer);
            return Err(eyre!("Failed to compile {} shader: {}", name, error));
        }
        Ok(())
    }

    unsafe fn check_program_link(program: u32) -> Result<()> {
        let mut success = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);
        if success == 0 {
            let mut len = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            let mut buffer = vec![0u8; len as usize];
            gl::GetProgramInfoLog(program, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut i8);
            let error = String::from_utf8_lossy(&buffer);
            return Err(eyre!("Failed to link shader program: {}", error));
        }
        Ok(())
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.program);
        }
    }
}

/// Texture handle
pub struct Texture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

impl Texture {
    pub fn from_rgba(data: &[u8], width: u32, height: u32) -> Result<Self> {
        let mut id = 0;
        unsafe {
            gl::GenTextures(1, &mut id);
            gl::BindTexture(gl::TEXTURE_2D, id);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );

            gl::BindTexture(gl::TEXTURE_2D, 0);
        }

        debug!("Created texture {} ({}x{})", id, width, height);
        Ok(Self { id, width, height })
    }

    /// Create a solid color texture (for testing/fallback)
    pub fn solid_color(r: u8, g: u8, b: u8) -> Result<Self> {
        let data = [r, g, b, 255u8];
        Self::from_rgba(&data, 1, 1)
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.id);
        }
    }
}

/// Vertex buffer for a fullscreen quad
pub struct QuadBuffer {
    vbo: u32,
}

impl QuadBuffer {
    pub fn new() -> Result<Self> {
        // Fullscreen quad vertices: position (x,y) + texcoord (u,v)
        #[rustfmt::skip]
        let vertices: [f32; 24] = [
            // Position    // TexCoord
            -1.0, -1.0,    0.0, 1.0,  // bottom-left
             1.0, -1.0,    1.0, 1.0,  // bottom-right
            -1.0,  1.0,    0.0, 0.0,  // top-left
             1.0, -1.0,    1.0, 1.0,  // bottom-right
             1.0,  1.0,    1.0, 0.0,  // top-right
            -1.0,  1.0,    0.0, 0.0,  // top-left
        ];

        let mut vbo = 0;
        unsafe {
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }

        Ok(Self { vbo })
    }

    pub fn bind(&self, shader: &ShaderProgram) {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            let stride = (4 * std::mem::size_of::<f32>()) as i32;

            // Position attribute
            gl::EnableVertexAttribArray(shader.a_position as u32);
            gl::VertexAttribPointer(
                shader.a_position as u32,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                ptr::null(),
            );

            // Texcoord attribute
            gl::EnableVertexAttribArray(shader.a_texcoord as u32);
            gl::VertexAttribPointer(
                shader.a_texcoord as u32,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );
        }
    }

    pub fn draw(&self) {
        unsafe {
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }
}

impl Drop for QuadBuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
        }
    }
}

/// Main renderer that manages wallpaper display and transitions
pub struct Renderer {
    shader: ShaderProgram,
    quad: QuadBuffer,
    current_texture: Option<Texture>,
    previous_texture: Option<Texture>,
    transition_type: TransitionType,
    transition_progress: f32,
    transition_time_ms: u32,
    background_mode: BackgroundMode,
    viewport_width: u32,
    viewport_height: u32,
}

impl Renderer {
    pub fn new(
        transition_type: TransitionType,
        transition_time_ms: u32,
        background_mode: BackgroundMode,
    ) -> Result<Self> {
        let shader = ShaderProgram::new()?;
        let quad = QuadBuffer::new()?;

        Ok(Self {
            shader,
            quad,
            current_texture: None,
            previous_texture: None,
            transition_type,
            transition_progress: 1.0, // Start with no transition
            transition_time_ms,
            background_mode,
            viewport_width: 0,
            viewport_height: 0,
        })
    }

    /// Set viewport size
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        unsafe {
            gl::Viewport(0, 0, width as i32, height as i32);
        }
    }

    /// Load a new wallpaper from RGBA data
    pub fn load_wallpaper(&mut self, data: &[u8], width: u32, height: u32) -> Result<()> {
        let new_texture = Texture::from_rgba(data, width, height)?;

        // Move current to previous for transition
        if self.current_texture.is_some() && self.transition_type != TransitionType::None {
            self.previous_texture = self.current_texture.take();
            self.transition_progress = 0.0;
        }

        self.current_texture = Some(new_texture);
        info!("Loaded new wallpaper ({}x{})", width, height);

        Ok(())
    }

    /// Load wallpaper from image file
    pub fn load_wallpaper_from_file(&mut self, path: &std::path::Path) -> Result<()> {
        info!("Loading wallpaper from: {:?}", path);

        let img = image::open(path)
            .wrap_err_with(|| format!("Failed to open image: {:?}", path))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        self.load_wallpaper(rgba.as_raw(), width, height)
    }

    /// Update transition progress
    pub fn update(&mut self, delta_ms: u32) -> bool {
        if self.transition_progress < 1.0 {
            let step = delta_ms as f32 / self.transition_time_ms as f32;
            self.transition_progress = (self.transition_progress + step).min(1.0);

            // Clean up previous texture when transition completes
            if self.transition_progress >= 1.0 {
                self.previous_texture = None;
            }

            true // Still animating
        } else {
            false // No animation
        }
    }

    /// Render the current wallpaper
    pub fn render(&self) {
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            // If no texture, just show black
            let Some(current) = &self.current_texture else {
                return;
            };

            gl::UseProgram(self.shader.program);

            // Bind current texture to unit 0
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, current.id);
            gl::Uniform1i(self.shader.u_texture, 0);

            // Bind previous texture to unit 1 (if transitioning)
            if let Some(prev) = &self.previous_texture {
                gl::ActiveTexture(gl::TEXTURE1);
                gl::BindTexture(gl::TEXTURE_2D, prev.id);
                gl::Uniform1i(self.shader.u_texture_prev, 1);
            }

            // Set uniforms
            gl::Uniform1f(self.shader.u_progress, self.transition_progress);
            gl::Uniform1i(
                self.shader.u_transition_type,
                self.transition_type_to_int(),
            );

            // Draw fullscreen quad
            self.quad.bind(&self.shader);
            self.quad.draw();

            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
        }
    }

    fn transition_type_to_int(&self) -> i32 {
        match self.transition_type {
            TransitionType::None => 0,
            TransitionType::Fade => 1,
            TransitionType::Slide => 2, // slide left
            TransitionType::Wipe => 2,  // same as slide for now
            TransitionType::Crossfade => 1, // same as fade
        }
    }

    /// Check if currently in a transition
    pub fn is_transitioning(&self) -> bool {
        self.transition_progress < 1.0
    }

    /// Set a solid color as wallpaper (for testing)
    pub fn set_solid_color(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        let texture = Texture::solid_color(r, g, b)?;
        self.current_texture = Some(texture);
        self.transition_progress = 1.0;
        Ok(())
    }
}
