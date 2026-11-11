//! Boxes with rounded or chamfered outer edges (README section 6.2).
//!
//! The wall follows the outer shape at `wall_m`: the inside is the same kind of box, each
//! dimension smaller by two walls, with the rounding radius smaller by one wall, or the
//! chamfer leg smaller by `wall * (2 - sqrt 2)` so the wall across a chamfer is `wall_m` too.
//! Chamfers cut all twelve edges at 45 degrees; where three meet, the corner is the point the
//! three cuts share. Mounts sit on the flat part of a face, and edge diffraction is taken
//! where that flat part ends.

use seatlock_core::registry::{MountPose, OuterShape};
use serde_json::{json, Value};

use crate::{box_face_facing, csg_difference, csg_hole, BoxShape};

#[derive(Clone, Copy, Debug)]
pub enum Edge {
    /// Radius of the rounding, m.
    Round(f64),
    /// Leg of the 45-degree chamfer along each face, m.
    Chamfer(f64),
}

impl Edge {
    fn size(self) -> f64 {
        match self {
            Edge::Round(r) | Edge::Chamfer(r) => r,
        }
    }

    /// The same edge on the inside of a wall `wall` thick.
    fn inside(self, wall: f64) -> Edge {
        match self {
            Edge::Round(r) => Edge::Round((r - wall).max(0.0)),
            Edge::Chamfer(c) => Edge::Chamfer((c - wall * (2.0 - 2f64.sqrt())).max(0.0)),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Edge::Round(_) => "rounding",
            Edge::Chamfer(_) => "chamfer",
        }
    }
}

/// Volume of an `a` x `b` x `c` box with every edge rounded or chamfered.
fn volume(size: [f64; 3], edge: Edge) -> f64 {
    let [a, b, c] = size;
    match edge {
        // The Minkowski sum of the core box and a sphere.
        Edge::Round(r) => {
            let (x, y, z) = (a - 2.0 * r, b - 2.0 * r, c - 2.0 * r);
            let pi = std::f64::consts::PI;
            x * y * z
                + 2.0 * r * (x * y + y * z + x * z)
                + pi * r * r * (x + y + z)
                + 4.0 / 3.0 * pi * r.powi(3)
        }
        // Twelve triangular prisms of area c^2/2 come off; at each corner three of them
        // overlap pairwise in c^3/3 and all together in c^3/4.
        Edge::Chamfer(k) => a * b * c - 2.0 * k * k * (a + b + c) + 6.0 * k.powi(3),
    }
}

/// True when `p` is inside the box from `min` to `max` with these edges.
fn inside(p: [f64; 3], min: [f64; 3], max: [f64; 3], edge: Edge) -> bool {
    for axis in 0..3 {
        if p[axis] <= min[axis] || p[axis] >= max[axis] {
            return false;
        }
    }
    match edge {
        Edge::Round(r) => {
            let mut sum = 0.0;
            for axis in 0..3 {
                let near = p[axis].clamp(min[axis] + r, max[axis] - r);
                sum += (p[axis] - near).powi(2);
            }
            sum < r * r
        }
        Edge::Chamfer(k) => {
            let half: Vec<f64> = (0..3).map(|i| 0.5 * (max[i] - min[i])).collect();
            let off: Vec<f64> = (0..3)
                .map(|i| (p[i] - 0.5 * (min[i] + max[i])).abs())
                .collect();
            (0..3).all(|i| {
                let j = (i + 1) % 3;
                off[i] + off[j] < half[i] + half[j] - k
            })
        }
    }
}

/// Points whose convex hull is the box: sphere octants at the corners for a rounding, the
/// four vertices of each corner for a chamfer.
fn hull_points(min: [f64; 3], max: [f64; 3], edge: Edge) -> Vec<[f64; 3]> {
    let mut points = Vec::new();
    for corner in 0..8 {
        let sign = [
            if corner & 1 == 0 { -1.0 } else { 1.0 },
            if corner & 2 == 0 { -1.0 } else { 1.0 },
            if corner & 4 == 0 { -1.0 } else { 1.0 },
        ];
        let at = |axis: usize| {
            if sign[axis] < 0.0 {
                min[axis]
            } else {
                max[axis]
            }
        };
        let pick = [at(0), at(1), at(2)];
        match edge {
            Edge::Round(r) => {
                let centre = [
                    pick[0] - sign[0] * r,
                    pick[1] - sign[1] * r,
                    pick[2] - sign[2] * r,
                ];
                if r <= 0.0 {
                    points.push(pick);
                    continue;
                }
                let n = 16;
                let half_pi = std::f64::consts::FRAC_PI_2;
                for i in 0..=n {
                    let polar = half_pi * i as f64 / n as f64;
                    let steps = if i == 0 { 0 } else { n };
                    for j in 0..=steps {
                        let around = half_pi * j as f64 / n as f64;
                        let dir = [
                            polar.sin() * around.cos(),
                            polar.sin() * around.sin(),
                            polar.cos(),
                        ];
                        points.push([
                            centre[0] + sign[0] * r * dir[0],
                            centre[1] + sign[1] * r * dir[1],
                            centre[2] + sign[2] * r * dir[2],
                        ]);
                    }
                }
            }
            Edge::Chamfer(k) => {
                if k <= 0.0 {
                    points.push(pick);
                    continue;
                }
                let step = |axis: usize, amount: f64| pick[axis] - sign[axis] * amount;
                points.push([pick[0], step(1, k), step(2, k)]);
                points.push([step(0, k), pick[1], step(2, k)]);
                points.push([step(0, k), step(1, k), pick[2]]);
                points.push([step(0, k / 2.0), step(1, k / 2.0), step(2, k / 2.0)]);
            }
        }
    }
    points
}

pub fn csg_hull(points: Vec<[f64; 3]>) -> Value {
    json!({ "op": "hull", "points_m": points })
}

pub struct EdgedBox {
    pub outer: BoxShape,
    pub edge: Edge,
}

impl EdgedBox {
    pub fn new(outer: BoxShape, edge: Edge) -> Result<EdgedBox, String> {
        let smallest = outer.width_m.min(outer.height_m).min(outer.depth_m);
        if edge.size() < 0.0 || 2.0 * edge.size() > smallest {
            return Err(format!(
                "The {} is larger than half the box's smallest side.",
                edge.word()
            ));
        }
        outer.interior_volume_m3()?;
        let inner = edge.inside(outer.wall_m);
        let wall = outer.wall_m;
        let inner_smallest = smallest - 2.0 * wall;
        if 2.0 * inner.size() > inner_smallest {
            return Err(format!(
                "The {} leaves no room inside the walls.",
                edge.word()
            ));
        }
        Ok(EdgedBox { outer, edge })
    }

    fn outer_size(&self) -> [f64; 3] {
        [self.outer.width_m, self.outer.depth_m, self.outer.height_m]
    }

    fn inner_bounds(&self) -> ([f64; 3], [f64; 3]) {
        let w = self.outer.wall_m;
        let [x, y, z] = self.outer_size();
        ([w, w, w], [x - w, y - w, z - w])
    }

    /// Width and height of a face, `[u, v]`, as `BoxShape::mount_pose` measures them.
    fn face_size(&self, surface: &str) -> Result<[f64; 2], String> {
        let (w, d, h) = (self.outer.width_m, self.outer.depth_m, self.outer.height_m);
        Ok(match surface {
            "front" | "back" => [w, h],
            "left" | "right" => [d, h],
            "top" | "bottom" => [w, d],
            other => return Err(format!("unknown face {other}")),
        })
    }
}

impl OuterShape for EdgedBox {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        let (min, max) = self.inner_bounds();
        let size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        Ok(volume(size, self.edge.inside(self.outer.wall_m)))
    }

    fn baffle_width_m(&self) -> f64 {
        self.outer.width_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        Ok(self.outer_size())
    }

    fn surfaces(&self) -> Vec<String> {
        OuterShape::surfaces(&self.outer)
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let [fu, fv] = self.face_size(surface)?;
        let e = self.edge.size();
        let [u, v] = at_m;
        if u < e || u > fu - e || v < e || v > fv - e {
            return Err(format!(
                "The mount at ({u:.3}, {v:.3}) m is off the flat part of the {surface} face, \
                 which starts {:.0} mm in from each edge.",
                e * 1000.0
            ));
        }
        self.outer.mount_pose(surface, at_m)
    }

    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        Some(box_face_facing(outward).to_string())
    }

    /// The flat part of the face: the box face moved in by the rounding or chamfer.
    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        let corners = OuterShape::surface_outline(&self.outer, surface)?;
        let mut centre = [0.0; 3];
        for corner in &corners {
            for axis in 0..3 {
                centre[axis] += corner[axis] / corners.len() as f64;
            }
        }
        let e = self.edge.size();
        Ok(corners
            .iter()
            .map(|corner| {
                let mut moved = *corner;
                for axis in 0..3 {
                    let towards = centre[axis] - corner[axis];
                    if towards.abs() > 1e-12 {
                        moved[axis] += e * towards.signum();
                    }
                }
                moved
            })
            .collect())
    }

    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        let (min, max) = self.inner_bounds();
        Ok(inside(
            point_m,
            min,
            max,
            self.edge.inside(self.outer.wall_m),
        ))
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.outer.wall_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, area)| area).sum();
        let outer = volume(self.outer_size(), self.edge);
        Ok((outer - self.interior_volume_m3()? - area * self.outer.wall_m).max(0.0))
    }

    fn notes(&self) -> Vec<String> {
        vec![format!(
            "Edges have a {:.0} mm {}. Edge diffraction is taken where the flat part of each face \
             ends; the softening a {} gives is not modeled.",
            self.edge.size() * 1000.0,
            self.edge.word(),
            self.edge.word()
        )]
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let (min, max) = self.inner_bounds();
        let mut parts = vec![
            csg_hull(hull_points([0.0; 3], self.outer_size(), self.edge)),
            csg_hull(hull_points(min, max, self.edge.inside(self.outer.wall_m))),
        ];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, self.outer.wall_m)),
        );
        Ok(csg_difference(parts))
    }

    fn contains_solid(&self, p: [f64; 3]) -> Result<bool, String> {
        let outer = inside(p, [0.0; 3], self.outer_size(), self.edge);
        Ok(outer && !self.contains_air(p)?)
    }

    fn air_tree(&self) -> Result<Value, String> {
        let (min, max) = self.inner_bounds();
        Ok(csg_hull(hull_points(
            min,
            max,
            self.edge.inside(self.outer.wall_m),
        )))
    }
}
