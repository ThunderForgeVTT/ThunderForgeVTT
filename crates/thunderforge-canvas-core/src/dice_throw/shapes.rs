//! The solids the dice are drawn as: vertex tables at unit scale, faces with
//! outward winding, and the label and value on each face (data-model.md,
//! `ShapeKind`). No textures: a face's number is drawn as text at its
//! projected centre (FR-006).

use glam::Vec3;

use super::ShapeKind;

/// One face: its vertices in counter-clockwise order seen from outside, its
/// label, and the value it shows, if any. A disc's rim has no value.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub indices: Vec<usize>,
    pub label: String,
    pub value: Option<i64>,
}

/// A convex solid, centred on the origin, every vertex within the unit
/// sphere.
#[derive(Debug, Clone, PartialEq)]
pub struct Solid {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
}

impl Solid {
    /// The face's outward unit normal (Newell's method).
    pub fn normal(&self, face: usize) -> Vec3 {
        let idx = &self.faces[face].indices;
        let mut n = Vec3::ZERO;
        for (i, &a) in idx.iter().enumerate() {
            let p = self.vertices[a];
            let q = self.vertices[idx[(i + 1) % idx.len()]];
            n.x += (p.y - q.y) * (p.z + q.z);
            n.y += (p.z - q.z) * (p.x + q.x);
            n.z += (p.x - q.x) * (p.y + q.y);
        }
        n.normalize()
    }

    pub fn centroid(&self, face: usize) -> Vec3 {
        let idx = &self.faces[face].indices;
        idx.iter().map(|&i| self.vertices[i]).sum::<Vec3>() / idx.len() as f32
    }

    /// The first face showing `value`.
    pub fn face_of(&self, value: i64) -> Option<usize> {
        self.faces.iter().position(|f| f.value == Some(value))
    }
}

/// The meshes a die of `kind` is drawn as when it shows `value`, each with
/// the face that ends toward the viewer. One for every kind but a d100, which
/// is tens then units. A value the shape has no face for is drawn as a disc
/// showing it (contracts/engine-dice.md).
pub fn build(kind: ShapeKind, value: i64) -> Vec<(Solid, usize)> {
    let one = |solid: Solid| -> Vec<(Solid, usize)> {
        match solid.face_of(value) {
            Some(face) => vec![(solid, face)],
            None => vec![disc_showing(value)],
        }
    };
    match kind {
        ShapeKind::D4 => one(tetrahedron()),
        ShapeKind::D6 => one(cube(&numbered(1, 6))),
        ShapeKind::D8 => one(octahedron()),
        ShapeKind::D10 => one(trapezohedron(&d10_faces())),
        ShapeKind::D12 => one(dodecahedron()),
        ShapeKind::D20 => one(icosahedron()),
        ShapeKind::D3 => one(cube(&[1, 2, 3, 1, 2, 3].map(|v| (v.to_string(), v)))),
        ShapeKind::Fate => one(cube(&fate_faces())),
        ShapeKind::Coin => one(disc(("H", 1), ("T", 0))),
        ShapeKind::D2 => one(disc(("1", 1), ("2", 2))),
        ShapeKind::D100 => {
            if !(1..=100).contains(&value) {
                return vec![disc_showing(value)];
            }
            let tens = (value / 10) % 10;
            let units = value % 10;
            let tens_die =
                trapezohedron(&(0..10).map(|t| (format!("{t}0"), t)).collect::<Vec<_>>());
            let units_die = trapezohedron(&(0..10).map(|u| (u.to_string(), u)).collect::<Vec<_>>());
            let tf = tens_die.face_of(tens).expect("0 to 9");
            let uf = units_die.face_of(units).expect("0 to 9");
            vec![(tens_die, tf), (units_die, uf)]
        }
        ShapeKind::Disc(_) => vec![disc_showing(value)],
    }
}

fn numbered(from: i64, to: i64) -> Vec<(String, i64)> {
    (from..=to).map(|v| (v.to_string(), v)).collect()
}

fn d10_faces() -> Vec<(String, i64)> {
    (1..=10).map(|v| ((v % 10).to_string(), v)).collect()
}

fn fate_faces() -> [(String, i64); 6] {
    [
        ("+".to_string(), 1),
        ("+".to_string(), 1),
        (String::new(), 0),
        (String::new(), 0),
        ("-".to_string(), -1),
        ("-".to_string(), -1),
    ]
}

fn disc_showing(value: i64) -> (Solid, usize) {
    let label = value.to_string();
    let solid = disc((label.as_str(), value), ("", i64::MIN));
    let face = solid.face_of(value).expect("the front shows it");
    (solid, face)
}

/// Builds faces from their outward normals: each face is every vertex
/// furthest along its normal, ordered counter-clockwise around it. Correct
/// for any convex solid whose faces are exactly those planes.
fn by_normals(vertices: Vec<Vec3>, normals: &[Vec3], labels: &[(String, i64)]) -> Solid {
    let faces = normals
        .iter()
        .zip(labels)
        .map(|(n, (label, value))| {
            let n = n.normalize();
            let top = vertices.iter().map(|v| v.dot(n)).fold(f32::MIN, f32::max);
            let mut idx: Vec<usize> = (0..vertices.len())
                .filter(|&i| (vertices[i].dot(n) - top).abs() < 1e-4)
                .collect();
            let centre = idx.iter().map(|&i| vertices[i]).sum::<Vec3>() / idx.len() as f32;
            let u = (vertices[idx[0]] - centre).normalize();
            let w = n.cross(u);
            idx.sort_by(|&a, &b| {
                let angle = |i: usize| {
                    let d = vertices[i] - centre;
                    d.dot(w).atan2(d.dot(u))
                };
                angle(a).total_cmp(&angle(b))
            });
            Face {
                indices: idx,
                label: label.clone(),
                value: Some(*value),
            }
        })
        .collect();
    Solid { vertices, faces }
}

/// Reverses any face whose winding points inward.
fn outward(mut solid: Solid) -> Solid {
    for f in 0..solid.faces.len() {
        if solid.normal(f).dot(solid.centroid(f)) < 0.0 {
            solid.faces[f].indices.reverse();
        }
    }
    solid
}

fn unit(points: Vec<Vec3>) -> Vec<Vec3> {
    let r = points.iter().map(|p| p.length()).fold(0.0, f32::max);
    points.into_iter().map(|p| p / r).collect()
}

fn signs3() -> impl Iterator<Item = Vec3> {
    [-1.0, 1.0].into_iter().flat_map(|x| {
        [-1.0, 1.0]
            .into_iter()
            .flat_map(move |y| [-1.0, 1.0].into_iter().map(move |z| Vec3::new(x, y, z)))
    })
}

const PHI: f32 = std::f32::consts::GOLDEN_RATIO;

fn icosahedron_points() -> Vec<Vec3> {
    let mut v = Vec::new();
    for a in [-1.0, 1.0] {
        for b in [-PHI, PHI] {
            v.push(Vec3::new(0.0, a, b));
            v.push(Vec3::new(a, b, 0.0));
            v.push(Vec3::new(b, 0.0, a));
        }
    }
    v
}

fn dodecahedron_points() -> Vec<Vec3> {
    let mut v: Vec<Vec3> = signs3().collect();
    let r = 1.0 / PHI;
    for a in [-r, r] {
        for b in [-PHI, PHI] {
            v.push(Vec3::new(0.0, a, b));
            v.push(Vec3::new(a, b, 0.0));
            v.push(Vec3::new(b, 0.0, a));
        }
    }
    v
}

pub fn tetrahedron() -> Solid {
    let vertices = unit(vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
    ]);
    let normals: Vec<Vec3> = vertices.iter().map(|v| -*v).collect();
    by_normals(vertices, &normals, &numbered(1, 4))
}

pub fn cube(labels: &[(String, i64)]) -> Solid {
    let vertices = unit(signs3().collect());
    // Opposite faces sum to seven on a d6.
    let normals = [Vec3::Z, Vec3::X, Vec3::Y, -Vec3::Y, -Vec3::X, -Vec3::Z];
    by_normals(vertices, &normals, labels)
}

pub fn octahedron() -> Solid {
    let vertices = vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let normals: Vec<Vec3> = signs3().collect();
    by_normals(vertices, &normals, &numbered(1, 8))
}

pub fn dodecahedron() -> Solid {
    let vertices = unit(dodecahedron_points());
    let normals = hull_normals(&vertices);
    by_normals(vertices, &normals, &numbered(1, 12))
}

pub fn icosahedron() -> Solid {
    let vertices = unit(icosahedron_points());
    let normals = hull_normals(&vertices);
    by_normals(vertices, &normals, &numbered(1, 20))
}

/// The outward normals of a convex point set's faces, found by trying every
/// plane through three of its points. Only for the two solids whose faces
/// are tedious to list by hand; twenty points is 1,140 planes.
fn hull_normals(vertices: &[Vec3]) -> Vec<Vec3> {
    let n = vertices.len();
    let mut normals: Vec<Vec3> = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                let (p, q, r) = (vertices[a], vertices[b], vertices[c]);
                let mut normal = (q - p).cross(r - p);
                if normal.length() < 1e-6 {
                    continue;
                }
                normal = normal.normalize();
                if normal.dot(p) < 0.0 {
                    normal = -normal;
                }
                let d = normal.dot(p);
                let bounds = vertices.iter().all(|v| normal.dot(*v) <= d + 1e-4);
                if bounds && !normals.iter().any(|m| m.dot(normal) > 1.0 - 1e-4) {
                    normals.push(normal);
                }
            }
        }
    }
    normals
}

/// A pentagonal trapezohedron: two apexes and two staggered rings of five,
/// with ring heights chosen so every kite is planar.
pub fn trapezohedron(labels: &[(String, i64)]) -> Solid {
    use std::f32::consts::TAU;
    let h = 1.0_f32;
    let c = (TAU / 10.0).cos();
    let z0 = h * (1.0 - c) / (1.0 + c);
    let mut vertices = vec![Vec3::new(0.0, 0.0, h), Vec3::new(0.0, 0.0, -h)];
    for k in 0..5 {
        let a = TAU * k as f32 / 5.0;
        vertices.push(Vec3::new(a.cos(), a.sin(), z0));
    }
    for k in 0..5 {
        let a = TAU * k as f32 / 5.0 + TAU / 10.0;
        vertices.push(Vec3::new(a.cos(), a.sin(), -z0));
    }
    let vertices = unit(vertices);
    let upper = |k: usize| 2 + k % 5;
    let lower = |k: usize| 7 + k % 5;
    let mut faces = Vec::new();
    for k in 0..5 {
        faces.push(vec![0, upper(k), lower(k), upper(k + 1)]);
        faces.push(vec![1, lower(k), upper(k + 1), lower(k + 1)]);
    }
    let faces = faces
        .into_iter()
        .zip(labels)
        .map(|(indices, (label, value))| Face {
            indices,
            label: label.clone(),
            value: Some(*value),
        })
        .collect();
    outward(Solid { vertices, faces })
}

/// A sixteen-sided prism: a front face, a back face and a rim.
pub fn disc(front: (&str, i64), back: (&str, i64)) -> Solid {
    use std::f32::consts::TAU;
    const N: usize = 16;
    let t = 0.2_f32;
    let mut vertices = Vec::with_capacity(2 * N);
    for z in [t, -t] {
        for k in 0..N {
            let a = TAU * k as f32 / N as f32;
            vertices.push(Vec3::new(a.cos(), a.sin(), z));
        }
    }
    let vertices = unit(vertices);
    let mut faces = vec![
        Face {
            indices: (0..N).collect(),
            label: front.0.to_string(),
            value: Some(front.1),
        },
        Face {
            indices: (N..2 * N).collect(),
            label: back.0.to_string(),
            value: (back.1 != i64::MIN).then_some(back.1),
        },
    ];
    for k in 0..N {
        let j = (k + 1) % N;
        faces.push(Face {
            indices: vec![k, N + k, N + j, j],
            label: String::new(),
            value: None,
        });
    }
    outward(Solid { vertices, faces })
}

#[cfg(test)]
#[path = "shapes_tests.rs"]
mod tests;
