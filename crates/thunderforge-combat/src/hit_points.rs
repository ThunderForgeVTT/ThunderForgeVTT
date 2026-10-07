//! Damage and healing, the arithmetic (spec 046 C7).
//!
//! The server's `combat::hit_points` locks the record, calls these, validates
//! and writes; the demo calls the same three on a creature it holds in memory.

use crate::manifest::SystemHitPoints;

/// Which way a change goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[cfg_attr(feature = "graphql", graphql(name = "HitPointChange"))]
pub enum HitPointChangeKind {
    Damage,
    Healing,
}

/// A creature's hit points, as the pack declares them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HitPoints {
    pub current: i32,
    pub max: i32,
    pub temporary: i32,
}

/// Why a combatant is out of the fight (`world_combatants.downed_by`).
pub const DOWNED_BY_HIT_POINTS: &str = "hit_points";
pub const DOWNED_BY_GAME_MASTER: &str = "game_master";

/// The arithmetic, and nothing else (C7).
///
/// Damage spends temporary hit points first and stops current at zero; the
/// excess is not carried anywhere (spec edge case "damage beyond zero").
/// Healing never exceeds the maximum and never touches temporary hit points,
/// which are granted rather than healed. A current already above the maximum
/// — which a sheet may hold — is not cut down by healing.
pub fn apply_to(before: HitPoints, kind: HitPointChangeKind, amount: i32) -> HitPoints {
    let amount = amount.max(0);
    match kind {
        HitPointChangeKind::Damage => {
            let temporary = before.temporary.max(0);
            let absorbed = temporary.min(amount);
            let rest = amount - absorbed;
            HitPoints {
                current: (before.current - rest).max(0),
                max: before.max,
                temporary: temporary - absorbed,
            }
        }
        HitPointChangeKind::Healing => HitPoints {
            current: if before.current >= before.max {
                before.current
            } else {
                before.current.saturating_add(amount).min(before.max)
            },
            max: before.max,
            temporary: before.temporary,
        },
    }
}

/// An integer field of a slot, clamped into `i32`.
pub fn read_int(slot: &serde_json::Value, field: &str) -> Option<i32> {
    slot.get(field)
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// Read the declared fields out of a slot's JSON.
///
/// A missing `current` reads as the maximum — a creature whose sheet names
/// only its maximum is at full health, which is what 5e's validator allows.
/// A missing maximum is not guessed at.
pub fn read_hit_points(
    slot: &serde_json::Value,
    declared: &SystemHitPoints,
) -> Result<HitPoints, String> {
    let max = read_int(slot, &declared.max)
        .ok_or_else(|| "That creature has no maximum hit points recorded".to_string())?;
    Ok(HitPoints {
        current: read_int(slot, &declared.current).unwrap_or(max),
        max,
        temporary: declared
            .temporary
            .as_deref()
            .and_then(|field| read_int(slot, field))
            .unwrap_or(0),
    })
}

/// Write the declared fields back, leaving everything else in the slot alone.
pub fn write_hit_points(
    slot: &serde_json::Value,
    declared: &SystemHitPoints,
    value: HitPoints,
) -> serde_json::Value {
    let mut out = slot.as_object().cloned().unwrap_or_default();
    out.insert(declared.current.clone(), value.current.into());
    if let Some(field) = &declared.temporary
        && (out.contains_key(field) || value.temporary != 0)
    {
        out.insert(field.clone(), value.temporary.into());
    }
    serde_json::Value::Object(out)
}

/// What a change does to whether a combatant is in the fight (C8), as the
/// server's `follow_zero` updates its rows: at zero, a combatant still in the
/// fight is downed by hit points (one the Game Master took down stays theirs);
/// above zero, one hit points took out comes back (FR-022). Returns the new
/// `(active, downed_by)`, or `None` when nothing changes.
pub fn standing_after(
    current: i32,
    active: bool,
    downed_by: Option<&str>,
) -> Option<(bool, Option<&'static str>)> {
    if current <= 0 {
        active.then_some((false, Some(DOWNED_BY_HIT_POINTS)))
    } else {
        (downed_by == Some(DOWNED_BY_HIT_POINTS)).then_some((true, None))
    }
}
