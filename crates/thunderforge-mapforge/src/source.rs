//! Reads `examples/maps/*.dd2vtt` as if it were an object store.
//!
//! A `.dd2vtt` is JSON with one large base64 image and some geometry. Decoding
//! that per request would be wasteful — the files run to 5MB of base64 — so a
//! map is decoded once, on first request, and the decoded image plus its
//! pyramid are held in memory.
//!
//! In-memory rather than a disk cache on purpose: this is a development
//! service, the corpus is eight files, and a process restart picking up an
//! edited fixture immediately is worth more here than avoiding a re-decode.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::DynamicImage;

use crate::tiles::{Pyramid, TILE_SIZE};

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("map not found: {0}")]
    NotFound(String),
    #[error("failed to read map: {0}")]
    Read(String),
    #[error("failed to decode map image: {0}")]
    Decode(String),
}

/// One decoded map, ready to serve tiles from.
pub struct LoadedMap {
    pub name: String,
    /// Level 0. Every other level is derived from this by downscaling.
    pub image: DynamicImage,
    pub pyramid: Pyramid,
    pub pixels_per_grid: f32,
    /// Downscaled level images, built on first use.
    ///
    /// Added after measuring: resizing the full image per tile request made a
    /// level-3 tile take **7.7 seconds** against 0.09s for a level-0 tile,
    /// because Lanczos3 over 6144x3456 ran once per tile rather than once per
    /// level. That gap is the whole argument for pre-generating a pyramid
    /// instead of deriving it on demand — this cache is the in-process version
    /// of that conclusion.
    levels: Mutex<HashMap<u32, Arc<DynamicImage>>>,
}

impl LoadedMap {
    /// The image for `level`, downscaling and caching on first request.
    pub fn level_image(&self, level: u32) -> Option<Arc<DynamicImage>> {
        let info = *self.pyramid.level(level)?;

        if let Ok(cache) = self.levels.lock() {
            if let Some(existing) = cache.get(&level) {
                return Some(Arc::clone(existing));
            }
        }

        let image = Arc::new(if info.level == 0 {
            self.image.clone()
        } else {
            self.image.resize_exact(
                info.width,
                info.height,
                image::imageops::FilterType::Lanczos3,
            )
        });

        if let Ok(mut cache) = self.levels.lock() {
            cache.insert(level, Arc::clone(&image));
        }
        Some(image)
    }
}

/// The map corpus, loaded lazily.
#[derive(Clone)]
pub struct MapSource {
    root: PathBuf,
    loaded: Arc<Mutex<HashMap<String, Arc<LoadedMap>>>>,
}

impl MapSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            loaded: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Names of every available map, sorted.
    pub fn list(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()?.to_str()? != "dd2vtt" {
                    return None;
                }
                Some(path.file_stem()?.to_str()?.to_owned())
            })
            .collect();
        names.sort();
        names
    }

    /// Decodes a map, or returns the already-decoded one.
    pub fn load(&self, name: &str) -> Result<Arc<LoadedMap>, SourceError> {
        // Rejects `..` and any path separator, so a request can never read
        // outside the corpus directory. This service has no auth, which makes
        // path containment the only thing standing between it and the rest of
        // the filesystem.
        if name.is_empty()
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
        {
            return Err(SourceError::NotFound(name.to_owned()));
        }

        if let Ok(cache) = self.loaded.lock() {
            if let Some(existing) = cache.get(name) {
                return Ok(Arc::clone(existing));
            }
        }

        let path = self.root.join(format!("{name}.dd2vtt"));
        let loaded = Arc::new(Self::decode(&path, name)?);

        if let Ok(mut cache) = self.loaded.lock() {
            cache.insert(name.to_owned(), Arc::clone(&loaded));
        }
        Ok(loaded)
    }

    fn decode(path: &Path, name: &str) -> Result<LoadedMap, SourceError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| SourceError::NotFound(name.to_owned()))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| SourceError::Read(e.to_string()))?;

        let encoded = parsed
            .get("image")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SourceError::Read("no image field".into()))?;
        let bytes = BASE64
            .decode(encoded)
            .map_err(|e| SourceError::Decode(e.to_string()))?;

        // Format is sniffed rather than assumed: these files carry WebP *or*
        // PNG despite the shared extension, a distinction that has broken this
        // project's importer before.
        let image =
            image::load_from_memory(&bytes).map_err(|e| SourceError::Decode(e.to_string()))?;

        let pixels_per_grid = parsed
            .get("resolution")
            .and_then(|r| r.get("pixels_per_grid"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(128.0) as f32;

        let pyramid = Pyramid::describe(image.width(), image.height(), TILE_SIZE);

        Ok(LoadedMap {
            name: name.to_owned(),
            image,
            pyramid,
            pixels_per_grid,
            levels: Mutex::new(HashMap::new()),
        })
    }
}
