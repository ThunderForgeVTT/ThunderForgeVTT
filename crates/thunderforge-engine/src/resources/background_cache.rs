//! Keeps recently-used background images resident on the GPU.
//!
//! Bevy frees an image's texture when the last strong handle to it drops.
//! `sync_scene_background` despawns the old background sprite on every
//! scene change, and that sprite held the only handle — so leaving a scene
//! threw its texture away and returning re-uploaded it in full.
//!
//! That upload is not cheap: about 350ms per megapixel, all on one frame.
//! Measured on the example corpus before this cache existed — 5825ms to
//! open `dwarven-forge`, then 5056ms to return to it after a single switch
//! away. Re-selecting a map whose texture had never been released cost
//! 20ms, which is what this makes the common case.
//!
//! Bounded by pixels rather than entries, and by
//! `thunderforge_canvas_core::texture_budget`, which owns the policy and
//! its tests. This resource is only the handles.

use bevy::prelude::*;
use thunderforge_canvas_core::texture_budget::{DEFAULT_BUDGET_PIXELS, retain_within_budget};

/// One retained image: the handle that keeps it alive, and what it costs.
struct Retained {
    path: String,
    /// Never read, and that is the point: holding a strong handle is the
    /// entire mechanism. Dropping it is what frees the GPU texture, so this
    /// field's only job is to exist for as long as the entry does.
    #[allow(dead_code)]
    handle: Handle<Image>,
    pixels: u64,
}

/// Strong handles to recently-shown background images, most recent first.
#[derive(Resource)]
pub struct BackgroundTextureCache {
    retained: Vec<Retained>,
    budget_pixels: u64,
}

impl Default for BackgroundTextureCache {
    fn default() -> Self {
        Self {
            retained: Vec::new(),
            budget_pixels: DEFAULT_BUDGET_PIXELS,
        }
    }
}

impl BackgroundTextureCache {
    /// Records a background as most-recently-used, then evicts down to the
    /// budget.
    ///
    /// `pixels` is the image's own pixel count. An imported map's scene
    /// `width`/`height` are set from the art's real pixel dimensions
    /// (`map_import`), so they are the right measure of what the texture
    /// costs — unlike anything derived from the camera or the viewport.
    pub fn touch(&mut self, path: &str, handle: Handle<Image>, pixels: u64) {
        // Re-showing a retained map must promote it, not duplicate it.
        self.retained.retain(|entry| entry.path != path);
        self.retained.insert(
            0,
            Retained {
                path: path.to_owned(),
                handle,
                pixels,
            },
        );

        let costs: Vec<u64> = self.retained.iter().map(|entry| entry.pixels).collect();
        let keep = retain_within_budget(&costs, self.budget_pixels);
        self.retained.truncate(keep);
    }

    /// Paths currently held resident, most recent first. For tracing and
    /// tests; the handles themselves are deliberately not exposed, since
    /// nothing outside this cache should be extending their lifetime.
    pub fn resident_paths(&self) -> impl ExactSizeIterator<Item = &str> {
        self.retained.iter().map(|entry| entry.path.as_str())
    }

    /// Total pixels currently held resident.
    pub fn resident_pixels(&self) -> u64 {
        self.retained.iter().map(|entry| entry.pixels).sum()
    }
}
