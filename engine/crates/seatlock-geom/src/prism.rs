//! Outlines extruded along one axis (README section 6.2): `prism`, and `sloped`, which is a
//! side profile extruded across the cabinet's width.
//!
//! The wall follows the outline: the inside outline is every side of the outer one moved
//! inward by `wall_m`, and both ends are `wall_m` thick.

use seatlock_core::registry::{MountPose, OuterShape};
use serde_json::{json, Value};

use crate::{csg_difference, csg_hole};

/// The axis an outline is extruded along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    /// The outline is the front, in `[x, z]`, extruded back along `y`.
    Y,
    /// The outline is the left side, in `[y, z]`, extruded right along `x`.
    X,
}

impl Axis {
    fn world(self, a: f64, b: f64, t: f64) -> [f64; 3] {
        match self {
            Axis::Y => [a, t, b],
            Axis::X => [t, a, b],
        }
    }
}

/// Signed area, positive when counter-clockwise.
fn signed_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        / 2.0
}

fn segments_cross(p: [f64; 2], q: [f64; 2], r: [f64; 2], s: [f64; 2]) -> bool {
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let d1 = cross(r, s, p);
    let d2 = cross(r, s, q);
    let d3 = cross(p, q, r);
    let d4 = cross(p, q, s);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn is_simple(points: &[[f64; 2]]) -> bool {
    let n = points.len();
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_cross(
                points[i],
                points[(i + 1) % n],
                points[j],
                points[(j + 1) % n],
            ) {
                return false;
            }
        }
    }
    true
}

/// Every side of a counter-clockwise outline moved inward by `distance`; corners are where
/// the moved sides meet. Refused when a side would vanish or turn round.
pub fn inset(points: &[[f64; 2]], distance: f64) -> Result<Vec<[f64; 2]>, String> {
    let n = points.len();
    let too_thick = || "The wall is too thick for this outline.".to_string();
    let mut lines = Vec::with_capacity(n);
    for i in 0..n {
        let (p, q) = (points[i], points[(i + 1) % n]);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let length = (dx * dx + dy * dy).sqrt();
        if length < 1e-9 {
            return Err("The outline has two corners in the same place.".into());
        }
        let inward = [-dy / length, dx / length];
        let start = [p[0] + inward[0] * distance, p[1] + inward[1] * distance];
        lines.push((start, [dx, dy]));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p1, d1) = lines[(i + n - 1) % n];
        let (p2, d2) = lines[i];
        let denom = d1[0] * d2[1] - d1[1] * d2[0];
        if denom.abs() < 1e-12 {
            // Straight through: the corner just moves inward.
            out.push(p2);
            continue;
        }
        let t = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / denom;
        out.push([p1[0] + t * d1[0], p1[1] + t * d1[1]]);
    }
    for i in 0..n {
        let (p, q) = (out[i], out[(i + 1) % n]);
        let d = lines[i].1;
        if (q[0] - p[0]) * d[0] + (q[1] - p[1]) * d[1] <= 0.0 {
            return Err(too_thick());
        }
    }
    if signed_area(&out) <= 0.0 || !is_simple(&out) {
        return Err(too_thick());
    }
    Ok(out)
}

fn point_in_polygon(points: &[[f64; 2]], at: [f64; 2]) -> bool {
    let mut inside = false;
    let n = points.len();
    let mut j = n - 1;
    for i in 0..n {
        let (pi, pj) = (points[i], points[j]);
        if (pi[1] > at[1]) != (pj[1] > at[1])
            && at[0] < (pj[0] - pi[0]) * (at[1] - pi[1]) / (pj[1] - pi[1]) + pi[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    [v[0] / length, v[1] / length, v[2] / length]
}

/// The directions of a flat face's own coordinates `[u, v]`, as seen from outside: `v` runs
/// up the face and `u` to the right. A level face uses the box's convention: `u` along `x`,
/// and `v` toward the front on the top, toward the back on the bottom (README section 4).
pub fn face_axes(outward: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    if outward[2] > 0.999 {
        return ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]);
    }
    if outward[2] < -0.999 {
        return ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    }
    let up = unit([
        -outward[2] * outward[0],
        -outward[2] * outward[1],
        1.0 - outward[2] * outward[2],
    ]);
    let right = cross([-outward[0], -outward[1], -outward[2]], up);
    (right, up)
}

struct Face {
    name: String,
    outward: [f64; 3],
    corners: Vec<[f64; 3]>,
}

pub struct Prism {
    axis: Axis,
    /// Counter-clockwise, with the lowest `a` and `b` at 0.
    outline: Vec<[f64; 2]>,
    inside: Vec<[f64; 2]>,
    length_m: f64,
    wall_m: f64,
    faces: Vec<Face>,
    baffle_width_m: f64,
}

impl Prism {
    /// `ends` names the faces at `t = 0` and `t = length`; `sides`, when given, names the
    /// face on each side of the outline in order (otherwise `side-1`, `side-2`, ...).
    pub fn new(
        axis: Axis,
        outline: &[[f64; 2]],
        length_m: f64,
        wall_m: f64,
        ends: [&str; 2],
        sides: Option<&[&str]>,
    ) -> Result<Prism, String> {
        if outline.len() < 3 {
            return Err("The outline needs at least three corners.".into());
        }
        if !(length_m > 2.0 * wall_m && wall_m > 0.0) {
            return Err("The prism is shorter than its two end walls.".into());
        }
        let mut points = outline.to_vec();
        if signed_area(&points) < 0.0 {
            points.reverse();
        }
        if !is_simple(&points) || signed_area(&points) <= 0.0 {
            return Err("The outline crosses itself.".into());
        }
        let min_a = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let min_b = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        for p in &mut points {
            p[0] -= min_a;
            p[1] -= min_b;
        }
        let inside = inset(&points, wall_m)?;
        let n = points.len();
        let mut faces = Vec::new();
        let [front_dir, back_dir] = match axis {
            Axis::Y => [[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
            Axis::X => [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        };
        faces.push(Face {
            name: ends[0].to_string(),
            outward: front_dir,
            corners: points.iter().map(|p| axis.world(p[0], p[1], 0.0)).collect(),
        });
        faces.push(Face {
            name: ends[1].to_string(),
            outward: back_dir,
            corners: points
                .iter()
                .rev()
                .map(|p| axis.world(p[0], p[1], length_m))
                .collect(),
        });
        for i in 0..n {
            let (p, q) = (points[i], points[(i + 1) % n]);
            let (da, db) = (q[0] - p[0], q[1] - p[1]);
            let length = (da * da + db * db).sqrt();
            let normal = axis.world(db / length, -da / length, 0.0);
            let name = match sides {
                Some(names) => names
                    .get(i)
                    .map(|name| name.to_string())
                    .ok_or("A side has no name.")?,
                None => format!("side-{}", i + 1),
            };
            faces.push(Face {
                name,
                outward: normal,
                corners: vec![
                    axis.world(p[0], p[1], 0.0),
                    axis.world(q[0], q[1], 0.0),
                    axis.world(q[0], q[1], length_m),
                    axis.world(p[0], p[1], length_m),
                ],
            });
        }
        let baffle_width_m = match axis {
            Axis::Y => points.iter().map(|p| p[0]).fold(0.0, f64::max),
            Axis::X => length_m,
        };
        Ok(Prism {
            axis,
            outline: points,
            inside,
            length_m,
            wall_m,
            faces,
            baffle_width_m,
        })
    }

    fn face(&self, name: &str) -> Result<&Face, String> {
        self.faces
            .iter()
            .find(|face| face.name == name)
            .ok_or_else(|| format!("This shape has no {name} surface."))
    }

    /// The extruded outline from `t0` to `t1`, as a CSG node the page builds with
    /// manifold-3d: the outline in `[a, b]` is placed by `origin + a*u + b*v + s*w` for `s` from
    /// 0 to `length_m`, with `u x v = w` so the solid keeps its outward faces.
    fn csg(&self, outline: &[[f64; 2]], t0: f64, t1: f64) -> Value {
        let (u, v, w, origin) = match self.axis {
            Axis::Y => (
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, -1.0, 0.0],
                [0.0, t1, 0.0],
            ),
            Axis::X => (
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [t0, 0.0, 0.0],
            ),
        };
        json!({ "op": "prism", "outline_m": outline, "origin_m": origin, "u": u, "v": v,
                "w": w, "length_m": t1 - t0 })
    }
}

impl OuterShape for Prism {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(signed_area(&self.inside) * (self.length_m - 2.0 * self.wall_m))
    }

    fn baffle_width_m(&self) -> f64 {
        self.baffle_width_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        let a = self.outline.iter().map(|p| p[0]).fold(0.0, f64::max);
        let b = self.outline.iter().map(|p| p[1]).fold(0.0, f64::max);
        Ok(match self.axis {
            Axis::Y => [a, self.length_m, b],
            Axis::X => [self.length_m, a, b],
        })
    }

    fn surfaces(&self) -> Vec<String> {
        self.faces.iter().map(|face| face.name.clone()).collect()
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let face = self.face(surface)?;
        let (right, up) = face_axes(face.outward);
        let flat: Vec<[f64; 2]> = face
            .corners
            .iter()
            .map(|c| [dot(*c, right), dot(*c, up)])
            .collect();
        let min_u = flat.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let min_v = flat.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let (u, v) = (at_m[0] + min_u, at_m[1] + min_v);
        let mut polygon = flat.clone();
        if signed_area(&polygon) < 0.0 {
            polygon.reverse();
        }
        if !point_in_polygon(&polygon, [u, v]) {
            return Err(format!(
                "The mount at ({:.3}, {:.3}) m is outside the {surface} face.",
                at_m[0], at_m[1]
            ));
        }
        let depth = dot(face.corners[0], face.outward);
        let n = face.outward;
        Ok(MountPose {
            position_m: [
                right[0] * u + up[0] * v + n[0] * depth,
                right[1] * u + up[1] * v + n[1] * depth,
                right[2] * u + up[2] * v + n[2] * depth,
            ],
            outward: n,
        })
    }

    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        let best = self.faces.iter().max_by(|a, b| {
            dot(a.outward, outward)
                .partial_cmp(&dot(b.outward, outward))
                .unwrap()
        })?;
        (dot(best.outward, outward) > 0.7).then(|| best.name.clone())
    }

    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        Ok(self.face(surface)?.corners.clone())
    }

    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        let [x, y, z] = point_m;
        let (a, b, t) = match self.axis {
            Axis::Y => (x, z, y),
            Axis::X => (y, z, x),
        };
        Ok(t > self.wall_m
            && t < self.length_m - self.wall_m
            && point_in_polygon(&self.inside, [a, b]))
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.wall_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, area)| area).sum();
        let outer = signed_area(&self.outline) * self.length_m;
        Ok((outer - self.interior_volume_m3()? - area * self.wall_m).max(0.0))
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let mut parts = vec![
            self.csg(&self.outline, 0.0, self.length_m),
            self.csg(&self.inside, self.wall_m, self.length_m - self.wall_m),
        ];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, self.wall_m)),
        );
        Ok(csg_difference(parts))
    }

    fn contains_solid(&self, point_m: [f64; 3]) -> Result<bool, String> {
        let [x, y, z] = point_m;
        let (a, b, t) = match self.axis {
            Axis::Y => (x, z, y),
            Axis::X => (y, z, x),
        };
        let outer = t > 0.0 && t < self.length_m && point_in_polygon(&self.outline, [a, b]);
        Ok(outer && !self.contains_air(point_m)?)
    }

    fn air_tree(&self) -> Result<Value, String> {
        Ok(self.csg(&self.inside, self.wall_m, self.length_m - self.wall_m))
    }
}

/// A box whose front leans back: deeper at the bottom than at the top, with the back
/// vertical. Its side profile in `[y, z]` is extruded across the width.
pub fn sloped(
    width_m: f64,
    height_m: f64,
    depth_bottom_m: f64,
    depth_top_m: f64,
    wall_m: f64,
) -> Result<Prism, String> {
    if depth_bottom_m <= 0.0 || depth_top_m <= 0.0 || height_m <= 0.0 {
        return Err("The sloped box needs a height and two depths.".into());
    }
    let (db, dt, h) = (depth_bottom_m, depth_top_m, height_m);
    let profile = [[0.0, 0.0], [db, 0.0], [db, h], [db - dt, h]];
    Prism::new(
        Axis::X,
        &profile,
        width_m,
        wall_m,
        ["left", "right"],
        Some(&["bottom", "back", "top", "front"]),
    )
}
