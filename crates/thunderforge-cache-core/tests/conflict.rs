//! Spec 028 T069: the conflict rule must be total and never consult a clock.

use thunderforge_cache_core::conflict::{Contender, Role, Winner, resolve};

fn gm(seq: u64) -> Contender {
    Contender {
        role: Role::GameMaster,
        reconnect_seq: seq,
    }
}

fn player(seq: u64) -> Contender {
    Contender {
        role: Role::Player,
        reconnect_seq: seq,
    }
}

#[test]
fn gm_beats_player_regardless_of_reconnect_order() {
    // The asymmetry that makes `Applied -> Superseded` reachable: the player
    // can reconnect first, have their change applied, and still lose.
    assert_eq!(resolve(gm(100), player(1)), Winner::A);
    assert_eq!(resolve(player(1), gm(100)), Winner::B);
}

#[test]
fn same_role_earlier_reconnect_wins() {
    assert_eq!(resolve(player(1), player(2)), Winner::A);
    assert_eq!(resolve(player(2), player(1)), Winner::B);
    assert_eq!(resolve(gm(1), gm(2)), Winner::A);
}

#[test]
fn resolution_is_total_across_every_combination() {
    // No ties, no Option, no "it depends" — two clients showing different
    // results is what FR-040 forbids outright.
    let roles = [Role::GameMaster, Role::Player];
    for role_a in roles {
        for role_b in roles {
            for seq_a in 0..4u64 {
                for seq_b in 0..4u64 {
                    let a = Contender {
                        role: role_a,
                        reconnect_seq: seq_a,
                    };
                    let b = Contender {
                        role: role_b,
                        reconnect_seq: seq_b,
                    };
                    let _: Winner = resolve(a, b);
                }
            }
        }
    }
}

#[test]
fn resolution_is_antisymmetric() {
    // Swapping the arguments must swap the winner, or the outcome would
    // depend on which client the server happened to look at first.
    let roles = [Role::GameMaster, Role::Player];
    for role_a in roles {
        for role_b in roles {
            for seq_a in 0..4u64 {
                for seq_b in 0..4u64 {
                    if role_a == role_b && seq_a == seq_b {
                        continue; // genuinely identical contenders
                    }
                    let a = Contender {
                        role: role_a,
                        reconnect_seq: seq_a,
                    };
                    let b = Contender {
                        role: role_b,
                        reconnect_seq: seq_b,
                    };
                    let forward = resolve(a, b);
                    let backward = resolve(b, a);
                    assert_ne!(
                        forward == Winner::A,
                        backward == Winner::A,
                        "resolve({a:?}, {b:?}) must invert when swapped"
                    );
                }
            }
        }
    }
}
