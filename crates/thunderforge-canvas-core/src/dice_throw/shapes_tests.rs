//! Spec 083 FR-006: every die is a real solid with its numbers on its faces.

use super::*;

fn values(solid: &Solid) -> Vec<i64> {
    solid.faces.iter().filter_map(|f| f.value).collect()
}

fn labels(solid: &Solid) -> Vec<&str> {
    solid.faces.iter().map(|f| f.label.as_str()).collect()
}

fn assert_convex_and_outward(solid: &Solid) {
    let centre = solid.vertices.iter().sum::<Vec3>() / solid.vertices.len() as f32;
    assert!(centre.length() < 1e-4, "centred: {centre:?}");
    for f in 0..solid.faces.len() {
        let n = solid.normal(f);
        assert!(
            n.dot(solid.centroid(f) - centre) > 0.0,
            "face {f} points out"
        );
        let idx = &solid.faces[f].indices;
        assert!(idx.len() >= 3);
        // Planar: every vertex on the face's plane.
        let d = n.dot(solid.vertices[idx[0]]);
        for &i in idx {
            assert!(
                (n.dot(solid.vertices[i]) - d).abs() < 1e-4,
                "face {f} is planar"
            );
        }
        // Convex, and wound counter-clockwise from outside.
        for k in 0..idx.len() {
            let a = solid.vertices[idx[k]];
            let b = solid.vertices[idx[(k + 1) % idx.len()]];
            let c = solid.vertices[idx[(k + 2) % idx.len()]];
            assert!((b - a).cross(c - b).dot(n) > 0.0, "face {f} is convex");
        }
        // Every other vertex is behind the face's plane.
        for v in &solid.vertices {
            assert!(n.dot(*v) <= d + 1e-4);
        }
    }
    for v in &solid.vertices {
        assert!(v.length() <= 1.0 + 1e-5, "unit scale");
    }
    let max = solid
        .vertices
        .iter()
        .map(|v| v.length())
        .fold(0.0, f32::max);
    assert!((max - 1.0).abs() < 1e-5, "reaches the unit sphere");
}

fn one(kind: ShapeKind, value: i64) -> Solid {
    let mut built = build(kind, value);
    assert_eq!(built.len(), 1);
    built.remove(0).0
}

#[test]
fn the_icosahedron_has_twenty_faces_numbered_once_each() {
    let s = icosahedron();
    assert_eq!(s.faces.len(), 20);
    assert_eq!(s.vertices.len(), 12);
    let mut v = values(&s);
    v.sort();
    assert_eq!(v, (1..=20).collect::<Vec<_>>());
    assert!(s.faces.iter().all(|f| f.indices.len() == 3));
    assert_convex_and_outward(&s);
}

#[test]
fn every_solid_is_convex_planar_outward_and_unit() {
    for kind in [
        ShapeKind::D4,
        ShapeKind::D6,
        ShapeKind::D8,
        ShapeKind::D10,
        ShapeKind::D12,
        ShapeKind::D20,
        ShapeKind::D3,
        ShapeKind::Fate,
        ShapeKind::Coin,
        ShapeKind::D2,
        ShapeKind::Disc(7),
    ] {
        assert_convex_and_outward(&one(kind, 1));
    }
    for (s, _) in build(ShapeKind::D100, 47) {
        assert_convex_and_outward(&s);
    }
}

#[test]
fn face_counts_and_labels() {
    for (kind, faces, sides) in [
        (ShapeKind::D4, 4, 3),
        (ShapeKind::D6, 6, 4),
        (ShapeKind::D8, 8, 3),
        (ShapeKind::D10, 10, 4),
        (ShapeKind::D12, 12, 5),
    ] {
        let s = one(kind, 1);
        assert_eq!(s.faces.len(), faces, "{kind:?}");
        assert!(s.faces.iter().all(|f| f.indices.len() == sides), "{kind:?}");
        let mut v = values(&s);
        v.sort();
        assert_eq!(v, (1..=faces as i64).collect::<Vec<_>>(), "{kind:?}");
    }
}

#[test]
fn a_d10_reads_zero_for_ten() {
    let s = one(ShapeKind::D10, 10);
    let ten = s.face_of(10).unwrap();
    assert_eq!(s.faces[ten].label, "0");
    assert_eq!(s.faces[s.face_of(3).unwrap()].label, "3");
}

#[test]
fn a_d100_is_tens_and_units() {
    let built = build(ShapeKind::D100, 47);
    assert_eq!(built.len(), 2);
    let (tens, tf) = &built[0];
    let (units, uf) = &built[1];
    let mut tl = labels(tens);
    tl.sort();
    assert_eq!(
        tl,
        vec!["00", "10", "20", "30", "40", "50", "60", "70", "80", "90"]
    );
    let mut ul = labels(units);
    ul.sort();
    assert_eq!(ul, vec!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    assert_eq!(tens.faces[*tf].label, "40");
    assert_eq!(units.faces[*uf].label, "7");

    let hundred = build(ShapeKind::D100, 100);
    assert_eq!(hundred[0].0.faces[hundred[0].1].label, "00");
    assert_eq!(hundred[1].0.faces[hundred[1].1].label, "0");
    let ten = build(ShapeKind::D100, 10);
    assert_eq!(ten[0].0.faces[ten[0].1].label, "10");
    assert_eq!(ten[1].0.faces[ten[1].1].label, "0");
}

#[test]
fn a_d3_and_a_fate_die_are_cubes() {
    let d3 = one(ShapeKind::D3, 2);
    assert_eq!(labels(&d3), vec!["1", "2", "3", "1", "2", "3"]);
    let fate = one(ShapeKind::Fate, 0);
    assert_eq!(labels(&fate), vec!["+", "+", "", "", "-", "-"]);
    assert_eq!(values(&fate), vec![1, 1, 0, 0, -1, -1]);
}

#[test]
fn coins_and_odd_sizes_are_discs() {
    let coin = one(ShapeKind::Coin, 0);
    assert_eq!(coin.faces[coin.face_of(1).unwrap()].label, "H");
    assert_eq!(coin.faces[coin.face_of(0).unwrap()].label, "T");
    let d2 = one(ShapeKind::D2, 2);
    assert_eq!(d2.faces[d2.face_of(2).unwrap()].label, "2");
    let d7 = one(ShapeKind::Disc(7), 7);
    assert_eq!(d7.faces[d7.face_of(7).unwrap()].label, "7");
    assert_eq!(d7.faces.len(), 18);
}

#[test]
fn a_value_the_shape_lacks_is_a_disc_showing_it() {
    let (s, f) = build(ShapeKind::D6, 9).remove(0);
    assert_eq!(s.faces[f].label, "9");
    assert_eq!(s.faces.len(), 18);
}
