//! A bounded ring of per-frame timings, with events attributed to the frame
//! they happened on.
//!
//! This exists to answer "did *that* hitch?", which an average cannot. A
//! smoothed frame time is the right number for "is the engine near its
//! budget" and the wrong one for "did swapping the map drop a frame": a
//! single 200ms stall inside a 60-sample moving average moves the average by
//! about 3ms, which is indistinguishable from noise. Sampling from outside
//! the engine cannot recover it either — a poll is itself work scheduled on
//! the frame loop, so the one frame that stalled is exactly the frame no
//! poll runs during.
//!
//! So every frame is recorded and the whole window is read afterwards. The
//! trace either contains the hitch or proves there wasn't one.
//!
//! The engine crate owns the clock and the ECS wiring
//! (`plugins/frame_trace.rs`); everything decidable without them is here,
//! where it can be tested.

use std::collections::VecDeque;

/// One frame's timing, plus whatever notable happened during it.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSample {
    /// Frames since the trace began. Retained across eviction, so a gap in
    /// this sequence is visible even after older samples are discarded.
    pub frame: u64,
    /// Real time since the previous frame, in milliseconds.
    pub dt_ms: f32,
    /// Events attributed to this frame, e.g. `background_spawn`. Usually
    /// empty; this is what makes a trace readable rather than a wall of
    /// numbers.
    pub marks: Vec<String>,
}

/// A fixed-capacity ring of [`FrameSample`]s.
///
/// Never allocates after construction in steady state: the deque is built at
/// capacity and eviction reuses its storage.
#[derive(Debug, Clone)]
pub struct FrameTrace {
    capacity: usize,
    samples: VecDeque<FrameSample>,
    /// Marks recorded before the frame's sample exists. Whatever leaves a
    /// mark (an asset finishing its load, a background being swapped) runs
    /// during the frame; the sample is only pushed at the end of it.
    pending: Vec<String>,
    frame: u64,
}

impl FrameTrace {
    /// A trace retaining at most `capacity` frames. A zero capacity is
    /// treated as one, so `record` always has somewhere to put a sample.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            samples: VecDeque::with_capacity(capacity),
            pending: Vec::new(),
            frame: 0,
        }
    }

    /// Attributes an event to the frame currently in progress.
    pub fn mark(&mut self, mark: impl Into<String>) {
        self.pending.push(mark.into());
    }

    /// Closes the current frame at `dt_ms`, evicting the oldest sample if
    /// the ring is full.
    pub fn record(&mut self, dt_ms: f32) {
        self.frame += 1;

        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }

        self.samples.push_back(FrameSample {
            frame: self.frame,
            dt_ms,
            // Drained, not copied: a mark belongs to one frame, and copying
            // would smear a single event across every frame after it.
            marks: std::mem::take(&mut self.pending),
        });
    }

    /// Discards every retained sample and any mark not yet attributed.
    ///
    /// The frame counter deliberately keeps counting: it identifies frames
    /// for the life of the engine, and resetting it would make two traces
    /// from one session claim the same frame numbers.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.pending.clear();
    }

    /// Retained samples, oldest first.
    pub fn samples(&self) -> impl ExactSizeIterator<Item = &FrameSample> {
        self.samples.iter()
    }

    /// The retained trace as a JSON array, oldest first.
    pub fn to_json(&self) -> String {
        let samples: Vec<serde_json::Value> = self
            .samples
            .iter()
            .map(|sample| {
                serde_json::json!({
                    "frame": sample.frame,
                    "dtMs": sample.dt_ms,
                    "marks": sample.marks,
                })
            })
            .collect();

        serde_json::to_string(&samples).unwrap_or_else(|_| "[]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_belongs_to_exactly_one_frame() {
        let mut trace = FrameTrace::new(8);

        trace.mark("background_spawn");
        trace.record(16.7);
        trace.record(16.7);

        let samples: Vec<&FrameSample> = trace.samples().collect();
        assert_eq!(samples[0].marks, vec!["background_spawn".to_string()]);
        assert!(
            samples[1].marks.is_empty(),
            "a mark must not smear onto later frames"
        );
    }

    #[test]
    fn several_marks_can_share_a_frame() {
        let mut trace = FrameTrace::new(8);

        trace.mark("background_spawn");
        trace.mark("background_loaded");
        trace.record(16.7);

        assert_eq!(
            trace.samples().next().unwrap().marks,
            vec![
                "background_spawn".to_string(),
                "background_loaded".to_string()
            ]
        );
    }

    #[test]
    fn the_ring_is_bounded_and_evicts_the_oldest() {
        let mut trace = FrameTrace::new(4);

        for frame in 1..=10 {
            trace.record(frame as f32);
        }

        let samples: Vec<&FrameSample> = trace.samples().collect();
        assert_eq!(samples.len(), 4);
        // Frames 7..=10 survive; 1..=6 were evicted from the front.
        assert_eq!(samples[0].frame, 7);
        assert_eq!(samples[3].frame, 10);
        assert_eq!(samples[0].dt_ms, 7.0);
    }

    #[test]
    fn a_hitch_survives_the_ring_intact() {
        // The whole point: an outlier must be readable as an outlier, not
        // averaged into its neighbours.
        let mut trace = FrameTrace::new(64);

        for _ in 0..30 {
            trace.record(16.7);
        }
        trace.mark("background_loaded");
        trace.record(184.2);
        for _ in 0..30 {
            trace.record(16.7);
        }

        let worst = trace
            .samples()
            .max_by(|a, b| a.dt_ms.total_cmp(&b.dt_ms))
            .unwrap();
        assert_eq!(worst.dt_ms, 184.2);
        assert_eq!(worst.marks, vec!["background_loaded".to_string()]);
    }

    #[test]
    fn clear_drops_samples_and_unattributed_marks_but_keeps_counting() {
        let mut trace = FrameTrace::new(8);

        trace.record(16.7);
        trace.mark("stale");
        trace.clear();
        trace.record(16.7);

        let samples: Vec<&FrameSample> = trace.samples().collect();
        assert_eq!(samples.len(), 1);
        assert!(
            samples[0].marks.is_empty(),
            "a mark left before clear() must not reappear after it"
        );
        assert_eq!(
            samples[0].frame, 2,
            "frame numbers identify frames for the session, so clear() must not reset them"
        );
    }

    #[test]
    fn zero_capacity_still_records() {
        let mut trace = FrameTrace::new(0);

        trace.record(16.7);
        trace.record(33.4);

        let samples: Vec<&FrameSample> = trace.samples().collect();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].dt_ms, 33.4);
    }

    #[test]
    fn json_uses_camel_case_keys_for_the_javascript_side() {
        let mut trace = FrameTrace::new(4);
        trace.mark("background_spawn");
        trace.record(16.5);

        let json: serde_json::Value = serde_json::from_str(&trace.to_json()).expect("valid JSON");
        let sample = &json.as_array().unwrap()[0];

        assert_eq!(sample["frame"], 1);
        assert_eq!(sample["dtMs"], 16.5);
        assert_eq!(sample["marks"][0], "background_spawn");
    }

    #[test]
    fn an_empty_trace_serializes_to_an_empty_array() {
        assert_eq!(FrameTrace::new(4).to_json(), "[]");
    }
}
