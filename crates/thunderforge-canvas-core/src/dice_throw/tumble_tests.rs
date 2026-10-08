//! Spec 083 FR-008, FR-009: the tumble is the same on every board and ends
//! where it lands.

use super::super::ShapeKind;
use super::super::landing::landing;
use super::super::shapes::build;
use super::*;

const ID: &str = "0d9b7a5c-0000-7000-8000-000000000001";

#[test]
fn the_seed_is_fnv1a() {
    assert_eq!(seed(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(seed("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(seed(ID), seed(ID));
    assert_ne!(seed(ID), seed("0d9b7a5c-0000-7000-8000-000000000002"));
}

#[test]
fn a_path_is_the_same_on_every_call() {
    let s = seed(ID);
    for index in 0..5 {
        assert_eq!(die_path(s, index, 5, 720.0), die_path(s, index, 5, 720.0));
    }
    let (solid, face) = build(ShapeKind::D20, 17).remove(0);
    let path = die_path(s, 0, 1, 720.0);
    let land = landing(&solid, face, path.spins[0]);
    for t in [0.0, 0.25, 0.5, 0.9] {
        assert_eq!(pose(&path, land, t), pose(&path, land, t));
    }
}

#[test]
fn at_the_end_it_is_at_rest_in_its_landing() {
    let s = seed(ID);
    let (solid, face) = build(ShapeKind::D20, 17).remove(0);
    let path = die_path(s, 0, 1, 720.0);
    let land = landing(&solid, face, path.spins[0]);
    let (at, turned) = pose(&path, land, 1.0);
    assert_eq!(at, path.rest);
    assert_eq!(turned, land);
    let (start, _) = pose(&path, land, 0.0);
    assert!((start - path.entry).length() < 1e-3);
}

#[test]
fn the_resting_place_does_not_depend_on_the_viewport() {
    let s = seed(ID);
    assert_eq!(
        die_path(s, 3, 6, 720.0).rest,
        die_path(s, 3, 6, 1080.0).rest
    );
}

#[test]
fn dice_of_one_throw_rest_apart() {
    let s = seed(ID);
    for count in [2, 5, 10, 11, 20] {
        let rests: Vec<_> = (0..count)
            .map(|i| die_path(s, i, count, 720.0).rest)
            .collect();
        for a in 0..count {
            for b in a + 1..count {
                assert!(
                    rests[a].distance(rests[b]) >= DIE_PX,
                    "{count} dice: {a} and {b} overlap"
                );
            }
        }
    }
}

#[test]
fn every_resting_place_is_in_the_lower_third() {
    let size = Vec2::new(1280.0, 720.0);
    for n in 0..20u32 {
        let s = seed(&format!("roll-{n}"));
        for count in [1, 7, 20] {
            for i in 0..count {
                let p = anchor(size) + die_path(s, i, count, size.y).rest;
                let half = DIE_PX / 2.0;
                assert!(
                    p.x - half >= -size.x / 2.0 && p.x + half <= size.x / 2.0,
                    "{p:?}"
                );
                assert!(
                    p.y - half >= -size.y / 2.0 && p.y + half <= -size.y / 6.0,
                    "{p:?}"
                );
            }
        }
    }
}

#[test]
fn turns_are_two_to_four() {
    for n in 0..50u32 {
        let p = die_path(seed(&n.to_string()), 0, 1, 720.0);
        assert!((2..=4).contains(&p.turns));
        assert!((p.axis.length() - 1.0).abs() < 1e-4);
    }
}
