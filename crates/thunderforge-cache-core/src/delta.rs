//! Computing what a client must fetch and discard.
//!
//! Spec 028 FR-007/FR-008/FR-015, contracts/graphql-delta-sync.md.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Fingerprint, HeldItem, ItemId};

/// One item the client must obtain.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PlanItem {
    pub id: ItemId,
    pub fingerprint: Fingerprint,
    pub byte_size: u64,
}

/// The server's answer to a client's manifest.
///
/// **Silence is meaningful.** An item the client holds that appears in
/// neither list is current, and saying nothing about it is what makes an
/// unchanged world nearly free to reopen.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SyncPlan {
    pub fetch: Vec<PlanItem>,
    pub evict: Vec<ItemId>,
}

/// What the server currently holds for one item, as far as *this caller* may
/// know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CurrentItem {
    /// `None` means the server has not yet computed a fingerprint — an asset
    /// row predating the backfill. Treated as "must fetch", never as
    /// unchanged: guessing "unchanged" would serve stale content forever, so
    /// the safe default is the expensive one.
    pub fingerprint: Option<Fingerprint>,
    pub byte_size: u64,
}

/// Compute the plan for one client.
///
/// `authorized_current` must contain **only** what the caller is permitted to
/// see. Filtering happens before this function rather than inside it, which
/// has two consequences worth stating: the function cannot leak an
/// unauthorized item even if it wanted to, and its tests need no auth
/// fixture at all.
///
/// Because unauthorized items are simply absent, a client's claim to hold one
/// falls through to the same branch as a deleted item — it lands in `evict`,
/// telling the client to discard it without revealing whether it still
/// exists. Cache correctness and permission revocation (FR-015) come out of
/// one mechanism rather than two.
///
/// Pure and total: same inputs, same plan, no ambient state, no clock.
pub fn compute_plan(
    held: &[HeldItem],
    authorized_current: &BTreeMap<ItemId, CurrentItem>,
) -> SyncPlan {
    let mut plan = SyncPlan::default();

    let held_by_id: BTreeMap<ItemId, Fingerprint> =
        held.iter().map(|h| (h.id, h.fingerprint)).collect();

    // Anything the caller holds that we cannot currently offer them — deleted,
    // or no longer permitted — must go.
    for id in held_by_id.keys() {
        if !authorized_current.contains_key(id) {
            plan.evict.push(*id);
        }
    }

    // Anything current whose fingerprint the caller does not already match.
    for (id, current) in authorized_current {
        match current.fingerprint {
            // No server fingerprint yet: fetch, never assume unchanged.
            None => plan.fetch.push(PlanItem {
                id: *id,
                fingerprint: Fingerprint::of_bytes(&[]),
                byte_size: current.byte_size,
            }),
            Some(server_fp) => match held_by_id.get(id) {
                // Matched — say nothing. This is the win.
                Some(client_fp) if *client_fp == server_fp => {}
                _ => plan.fetch.push(PlanItem {
                    id: *id,
                    fingerprint: server_fp,
                    byte_size: current.byte_size,
                }),
            },
        }
    }

    plan
}
