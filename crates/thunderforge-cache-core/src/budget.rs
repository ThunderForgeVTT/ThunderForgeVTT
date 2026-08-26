//! How much may be stored locally, and what goes when it is full.
//!
//! Spec 028 FR-022/FR-023, research.md R8.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ItemId;

/// Half of whatever the browser offers.
const QUOTA_SHARE_DENOMINATOR: u64 = 2;

/// Ceiling regardless of quota. Beyond this the marginal benefit is small and
/// the eviction bookkeeping is not free.
pub const MAX_BUDGET_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// One thing held locally, as the eviction planner sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: ItemId,
    pub world_id: Uuid,
    pub byte_size: u64,
    /// Monotonic counter, not a wall clock. Ordering is all that matters, and
    /// a counter cannot be wrong the way a clock can.
    pub last_read_seq: u64,
}

/// What to release to fit.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct BudgetPlan {
    pub limit_bytes: u64,
    pub in_use_bytes: u64,
    pub evict: Vec<ItemId>,
    /// True when even evicting everything permissible leaves too little room.
    /// The caller then fetches without storing (FR-024) — degraded, but
    /// correct, and never a failed load.
    pub insufficient: bool,
}

/// The budget for a machine reporting `reported_quota` bytes available.
///
/// Proportional rather than a shipped constant: a fixed number is wrong on a
/// low-storage laptop and absurd on a workstation. Taking only half leaves
/// headroom so this feature does not starve the application's other storage
/// (FR-022b).
pub fn limit_bytes(reported_quota: u64) -> u64 {
    (reported_quota / QUOTA_SHARE_DENOMINATOR).min(MAX_BUDGET_BYTES)
}

/// Choose what to release to fit `incoming` bytes within `limit`.
///
/// Two rules, in order:
///
/// 1. **Never the open world** (FR-023), even when that means returning a
///    plan that does not free enough. `insufficient` reports that honestly
///    rather than evicting content the user is actively looking at.
/// 2. **Whole worlds before individual items**, least-recently-used first. A
///    half-cached world is the worst outcome available — it is slow *and* it
///    occupies space — so worlds are released entire before any single-item
///    eviction is considered.
///
/// Deterministic throughout: ties break by world id then item id, so the same
/// index always yields the same plan and tests can assert on it exactly.
pub fn plan_eviction(
    index: &[IndexEntry],
    limit: u64,
    incoming: u64,
    open_world: Uuid,
) -> BudgetPlan {
    let in_use: u64 = index.iter().map(|e| e.byte_size).sum();
    let mut plan = BudgetPlan {
        limit_bytes: limit,
        in_use_bytes: in_use,
        ..Default::default()
    };

    if in_use + incoming <= limit {
        return plan;
    }
    let mut to_free = (in_use + incoming).saturating_sub(limit);

    // Worlds other than the open one, least-recently-used first. A world's
    // recency is that of its most recently read item — touching any part of a
    // world keeps the whole of it warm, which is what makes releasing it
    // entire the right unit.
    let mut worlds: Vec<(Uuid, u64, u64)> = Vec::new();
    for entry in index {
        if entry.world_id == open_world {
            continue;
        }
        match worlds.iter_mut().find(|(w, _, _)| *w == entry.world_id) {
            Some((_, size, recency)) => {
                *size += entry.byte_size;
                *recency = (*recency).max(entry.last_read_seq);
            }
            None => worlds.push((entry.world_id, entry.byte_size, entry.last_read_seq)),
        }
    }
    worlds.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));

    for (world_id, size, _) in worlds {
        if to_free == 0 {
            break;
        }
        let mut ids: Vec<ItemId> = index
            .iter()
            .filter(|e| e.world_id == world_id)
            .map(|e| e.id)
            .collect();
        ids.sort();
        plan.evict.extend(ids);
        to_free = to_free.saturating_sub(size);
    }

    // Still short. Everything left belongs to the open world, which rule 1
    // forbids touching — so say so instead of breaking it.
    plan.insufficient = to_free > 0;
    plan
}
