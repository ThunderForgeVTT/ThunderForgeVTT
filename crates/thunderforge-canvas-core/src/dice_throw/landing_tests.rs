//! Spec 083 FR-007: the face the die lands on is the server's.

use glam::Vec3;

use super::super::ShapeKind;
use super::super::shapes::build;
use super::*;

const SPINS: [f32; 4] = [0.0, 1.0, 2.5, 5.9];

fn lands_up(kind: ShapeKind, value: i64) {
    for (solid, face) in build(kind, value) {
        for spin in SPINS {
            let up = landing(&solid, face, spin) * solid.normal(face);
            assert!(
                (up - Vec3::Z).length() < 1e-5,
                "{kind:?} {value} spin {spin}: {up:?}"
            );
        }
    }
}

#[test]
fn every_d20_value_lands_face_up() {
    for value in 1..=20 {
        lands_up(ShapeKind::D20, value);
    }
}

#[test]
fn every_value_of_every_shape_lands_face_up() {
    for (kind, values) in [
        (ShapeKind::D4, (1..=4).collect::<Vec<_>>()),
        (ShapeKind::D6, (1..=6).collect()),
        (ShapeKind::D8, (1..=8).collect()),
        (ShapeKind::D10, (1..=10).collect()),
        (ShapeKind::D12, (1..=12).collect()),
        (ShapeKind::D100, (1..=100).collect()),
        (ShapeKind::D3, (1..=3).collect()),
        (ShapeKind::Fate, vec![-1, 0, 1]),
        (ShapeKind::Coin, vec![0, 1]),
        (ShapeKind::D2, vec![1, 2]),
        (ShapeKind::Disc(7), (1..=7).collect()),
    ] {
        for value in values {
            lands_up(kind, value);
        }
    }
}
