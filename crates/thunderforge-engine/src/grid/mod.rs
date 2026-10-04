//! Grid geometry lives in `thunderforge_canvas_core::grid` so its tests can
//! actually run — this crate only targets wasm and cannot link `winit`
//! natively, so `cargo test` here compile-checks but never executes.
//!
//! Re-exported for convenience; the Bevy `Resource` wrapper is
//! `crate::resources::SceneGrid`.

pub use thunderforge_canvas_core::grid::{Cell, GridKind, GridSpec};
