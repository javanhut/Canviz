use color_eyre::eyre::{eyre, Result, WrapErr};
use image::{DynamicImage, GenericImageView, ImageFormat};
use log::{debug, info};
use std::path::Path;

/// Loaded image data ready for GPU upload
pub struct ImageData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl ImageData {
    /// Load an image from a file path
    pub fn load(path: &Path) -> Result<Self> {
        info!("Loading image: {:?}", path);

        let img = image::open(path)
            .wrap_err_with(|| format!("Failed to open image: {:?}", path))?;

        let (width, height) = img.dimensions();
        debug!("Image dimensions: {}x{}", width, height);

        // Convert to RGBA8
        let rgba = img.to_rgba8().into_raw();

        Ok(Self { rgba, width, height })
    }

    /// Load an image from memory
    pub fn from_memory(data: &[u8]) -> Result<Self> {
        let img = image::load_from_memory(data)
            .wrap_err("Failed to decode image from memory")?;

        let (width, height) = img.dimensions();
        let rgba = img.to_rgba8().into_raw();

        Ok(Self { rgba, width, height })
    }

    /// Create a solid color image (for testing)
    pub fn solid_color(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let pixel = [r, g, b, a];
        let rgba: Vec<u8> = pixel
            .iter()
            .cycle()
            .take((width * height * 4) as usize)
            .copied()
            .collect();

        Self { rgba, width, height }
    }
}

/// Background image loader with caching
pub struct ImageLoader {
    // Could add LRU cache here for frequently used images
}

impl ImageLoader {
    pub fn new() -> Self {
        Self {}
    }

    /// Load an image, potentially from cache
    pub fn load(&self, path: &Path) -> Result<ImageData> {
        ImageData::load(path)
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Image picker for slideshow functionality
pub struct ImagePicker {
    images: Vec<std::path::PathBuf>,
    current_index: usize,
}

impl ImagePicker {
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            current_index: 0,
        }
    }

    /// Scan a directory for images
    pub fn scan_directory(&mut self, path: &Path, recursive: bool) -> Result<()> {
        self.images.clear();

        let supported_extensions = ["jpg", "jpeg", "png", "bmp", "gif", "webp"];

        if path.is_file() {
            self.images.push(path.to_path_buf());
            return Ok(());
        }

        if !path.is_dir() {
            return Err(eyre!("Path is neither a file nor directory: {:?}", path));
        }

        let walker = if recursive {
            walkdir::WalkDir::new(path)
        } else {
            walkdir::WalkDir::new(path).max_depth(1)
        };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    if supported_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str())
                    {
                        self.images.push(entry_path.to_path_buf());
                    }
                }
            }
        }

        info!("Found {} images in {:?}", self.images.len(), path);
        Ok(())
    }

    /// Get current image path
    pub fn current(&self) -> Option<&Path> {
        self.images.get(self.current_index).map(|p| p.as_path())
    }

    /// Move to next image
    pub fn next(&mut self) -> Option<&Path> {
        if self.images.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.images.len();
        self.current()
    }

    /// Move to previous image
    pub fn previous(&mut self) -> Option<&Path> {
        if self.images.is_empty() {
            return None;
        }
        self.current_index = if self.current_index == 0 {
            self.images.len() - 1
        } else {
            self.current_index - 1
        };
        self.current()
    }

    /// Shuffle images randomly
    pub fn shuffle(&mut self) {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let hasher = RandomState::new().build_hasher();
        let seed = hasher.finish();

        // Simple Fisher-Yates shuffle with pseudo-random
        let mut rng_state = seed;
        for i in (1..self.images.len()).rev() {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (rng_state as usize) % (i + 1);
            self.images.swap(i, j);
        }
    }

    /// Sort images by name
    pub fn sort_ascending(&mut self) {
        self.images.sort();
    }

    /// Sort images by name descending
    pub fn sort_descending(&mut self) {
        self.images.sort_by(|a, b| b.cmp(a));
    }

    /// Get total number of images
    pub fn count(&self) -> usize {
        self.images.len()
    }
}

impl Default for ImagePicker {
    fn default() -> Self {
        Self::new()
    }
}
