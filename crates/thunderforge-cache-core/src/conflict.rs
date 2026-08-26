//! Who wins when two disconnected clients changed the same thing.
//!
//! Spec 028 FR-040/FR-040a/FR-040b, ADR-052.

use serde::{Deserialize, Serialize};

/// A participant's role in the world, as the server derives it.
///
/// Never taken from a client: a `QueuedChange` carries a role *hint* so the
/// client can predict an outcome, but the server re-derives the real one at
/// reconnection and ignores what it was told.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Role {
    GameMaster,
    Player,
}

/// Position in the order clients reconnected. Monotonic, server-assigned.
///
/// Deliberately not a timestamp. Client clocks are forgeable and routinely
/// wrong, and a skewed one would silently overwrite other people's work —
/// which is exactly the failure a conflict rule exists to prevent.
pub type ReconnectSeq = u64;

/// Which of two conflicting changes stands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Winner {
    A,
    B,
}

/// One side of a conflict, reduced to what the rule actually needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Contender {
    pub role: Role,
    pub reconnect_seq: ReconnectSeq,
}

/// Resolve a conflict between two changes to the same item.
///
/// **GM beats player**, regardless of who reconnected first. That mirrors how
/// authority already works at a table, and it is easy to explain to the person
/// who loses — which matters, because someone always loses.
///
/// **Same role: earlier reconnection wins.** Deterministic without needing
/// clocks or coordination.
///
/// Total by construction: every pair resolves, there is no tie and no
/// `Option`. A rule that could return "it depends" would leave two clients
/// showing different results, which FR-040 forbids outright.
///
/// Shared by both sides on purpose: the server *decides* with this, and the
/// client *predicts* with it so the UI can say what will happen. Those two
/// answers must never differ, which is why the client must not reimplement
/// this locally for responsiveness (FR-040b).
pub fn resolve(a: Contender, b: Contender) -> Winner {
    match (a.role, b.role) {
        (Role::GameMaster, Role::Player) => Winner::A,
        (Role::Player, Role::GameMaster) => Winner::B,
        // Same role — whoever got back first.
        _ => {
            if a.reconnect_seq <= b.reconnect_seq {
                Winner::A
            } else {
                Winner::B
            }
        }
    }
}
