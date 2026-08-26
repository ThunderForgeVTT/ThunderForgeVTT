//! ECS wiring for the raw per-frame timing trace.
//!
//! The buffer, its eviction and its mark attribution live in
//! `thunderforge_canvas_core::frame_trace`, where they are tested — this
//! crate cannot be compiled for the host (winit has no backend under the
//! engine's feature set), so a `#[test]` here would never run. That is not
//! hypothetical: 194 tests in `integration_tests.rs` were silently dead for
//! several refactors for a related reason.
//!
//! What is genuinely engine-side is here: the clock, the schedule position,
//! and the static that lets `wasm_bindgen` reach a `World` owned by an
//! `App::run()` that never returns.
//!
//! Deltas come from `Time<Real>`, not `Time`. The default virtual clock
//! clamps its delta (`max_delta`, 250ms out of the box) precisely so game
//! logic is never handed an enormous timestep after a stall — which would
//! silently truncate a measurement to the clamp, reporting a one-second
//! freeze as 250ms.
//!
//! Always on: one push per frame into a buffer that does not grow.

use bevy::prelude::*;
use std::sync::{Mutex, OnceLock};
use thunderforge_canvas_core::frame_trace::FrameTrace;

/// Frames retained — about ten seconds at 60fps, which spans a map switch
/// (request, decode, upload) with quiet frames either side for a baseline.
const TRACE_CAPACITY: usize = 600;

fn trace() -> &'static Mutex<FrameTrace> {
    static TRACE: OnceLock<Mutex<FrameTrace>> = OnceLock::new();
    TRACE.get_or_init(|| Mutex::new(FrameTrace::new(TRACE_CAPACITY)))
}

/// Attributes an event to the frame currently being processed.
///
/// Call from anywhere in the schedule before `Last`. Intended for rare,
/// deliberate events — a map switch, an asset finishing its load — not
/// per-entity logging.
pub fn mark_frame(mark: impl Into<String>) {
    if let Ok(mut trace) = trace().lock() {
        trace.mark(mark);
    }
}

/// The retained trace, oldest frame first, as JSON.
pub fn frame_trace_json() -> String {
    trace()
        .lock()
        .map(|trace| trace.to_json())
        .unwrap_or_else(|_| "[]".to_string())
}

/// Drops every retained sample. Call immediately before the thing being
/// measured so the window contains it and nothing else.
pub fn clear_frame_trace() {
    if let Ok(mut trace) = trace().lock() {
        trace.clear();
    }
}

fn record_frame(time: Res<Time<Real>>) {
    let dt_ms = time.delta_secs_f64() as f32 * 1000.0;
    if let Ok(mut trace) = trace().lock() {
        trace.record(dt_ms);
    }
}

pub struct FrameTracePlugin;

impl Plugin for FrameTracePlugin {
    fn build(&self, app: &mut App) {
        // `Last`, so the sample covers the whole frame's work and a mark
        // left by an `Update` system lands on the frame it happened in
        // rather than the next one.
        app.add_systems(Last, record_frame);
    }
}
