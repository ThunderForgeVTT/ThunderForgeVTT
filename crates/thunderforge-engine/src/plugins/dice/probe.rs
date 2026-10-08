//! What the engine reports about the throws it drew (research R10,
//! contracts/engine-dice.md). The e2e reads it through `dice_landed()`,
//! `dice_timings()` and `dice_entity_count()`, exported from `sdk.rs`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use serde::Serialize;
use thunderforge_canvas_core::dice_throw::{TIMINGS, ThrowDie};
use thunderforge_dice::DieSides;

use super::DiceThrowEntity;

/// The landed log keeps this many entries, dropping the oldest.
pub const LOG_CAP: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandedThrow {
    pub roll_id: String,
    pub skipped: bool,
    pub reduced_motion: bool,
    pub readout: Option<String>,
    pub chip: Option<String>,
    pub dice: Vec<LandedDie>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandedDie {
    /// A number, or `"F"` for Fate and `"C"` for a coin.
    pub sides: serde_json::Value,
    pub face: i64,
    pub kept: bool,
    pub succeeded: Option<bool>,
    pub rerolled: Vec<i64>,
    pub clamped: Option<i64>,
    pub explosion_of: Option<usize>,
    pub resting_place: [i64; 2],
}

impl LandedDie {
    pub fn of(die: &ThrowDie, sides: DieSides, rest: Vec2) -> Self {
        Self {
            sides: match sides {
                DieSides::Numeric(n) => serde_json::Value::from(n),
                DieSides::Fate => serde_json::Value::from("F"),
                DieSides::Coin => serde_json::Value::from("C"),
            },
            face: die.face(),
            kept: die.kept,
            succeeded: die.succeeded,
            rerolled: die.rerolled(),
            clamped: die.clamped(),
            explosion_of: die.explosion_of,
            resting_place: [rest.x.round() as i64, rest.y.round() as i64],
        }
    }
}

fn log() -> &'static Mutex<VecDeque<LandedThrow>> {
    static LOG: OnceLock<Mutex<VecDeque<LandedThrow>>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(VecDeque::new()))
}

static ENTITIES: AtomicU32 = AtomicU32::new(0);

/// Writes an entry, dropping the oldest past [`LOG_CAP`].
pub fn record(entry: LandedThrow) {
    if let Ok(mut log) = log().lock() {
        log.push_back(entry);
        while log.len() > LOG_CAP {
            log.pop_front();
        }
    }
}

pub fn landed_json() -> String {
    log()
        .lock()
        .ok()
        .and_then(|log| serde_json::to_string(&*log).ok())
        .unwrap_or_else(|| "[]".to_string())
}

pub fn timings_json() -> String {
    let t = TIMINGS;
    format!(
        "{{\"tumbleMs\":{},\"stepMs\":{},\"holdMs\":{},\"fadeMs\":{},\"reducedMs\":{}}}",
        t.tumble_ms, t.step_ms, t.hold_ms, t.fade_ms, t.reduced_ms
    )
}

pub fn entity_count() -> u32 {
    ENTITIES.load(Ordering::Relaxed)
}

pub(super) fn count_entities(entities: Query<(), With<DiceThrowEntity>>) {
    ENTITIES.store(entities.iter().count() as u32, Ordering::Relaxed);
}
