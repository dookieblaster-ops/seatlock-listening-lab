//! The outer shape `revolved` (README section 6.2): a curved body of revolution about the
//! front-to-back axis, with a flat round front for a driver and a flat round back. Cylinders,
//! bullets, eggs, and cut-off spheres are all members.
//!
//! ```text
//! { "kind": "revolved", "diameter_m": 0.22, "length_m": 0.26, "front_diameter_m": 0.15,
//!   "back_diameter_m": 0.08, "widest_at": 0.4, "wall_m": 0.008 }
//! ```
//!
//! The outside radius runs from the front's rim to the widest point, `widest_at` of the way
//! back, along a quarter ellipse, and from there to the back's rim along another. The wall
//! follows the outside at `wall_m`: the inside is every point at least that far from the
//! outer surface.

use seatlock_core::registry::{MountPose, OuterShape};
use serde_json::{json, Map, Value};

use crate::{csg_difference, csg_hole};

/// Points along the outside profile, and along the inside one.
const PROFILE_POINTS: usize = 96;

pub struct Revolved {
    radius_m: f64,
    length_m: f64,
    front_radius_m: f64,
    back_radius_m: f64,
    widest_at: f64,
    wall_m: f64,
    /// The outside profile `[y, r]`, front rim to back rim.
    outer: Vec<[f64; 2]>,
    /// The inside profile `[y, r]`, from `wall_m` behind the front to `wall_m` before the back.
    inner: Vec<[f64; 2]>,
}

fn number(fields: &Map<String, Value>, key: &str) -> Result<f64, String> {
    fields
        .get(key)
        .and_then(Value::as_f64)
        .ok_or(format!("missing {key}"))
}

pub fn parse(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let radius_m = number(fields, "diameter_m")? / 2.0;
    let length_m = number(fields, "length_m")?;
    let wall_m = number(fields, "wall_m")?;
    let front_radius_m = number(fields, "front_diameter_m")? / 2.0;
    let back_radius_m = fields
        .get("back_diameter_m")
        .and_then(Value::as_f64)
        .map(|d| d / 2.0)
        .unwrap_or(front_radius_m);
    let widest_at = fields
        .get("widest_at")
        .and_then(Value::as_f64)
        .unwrap_or(0.5)
        .clamp(0.05, 0.95);
    if !(radius_m > 0.0 && length_m > 0.0 && wall_m > 0.0) {
        return Err("A revolved shape needs a diameter, a length, and a wall.".into());
    }
    if front_radius_m > radius_m + 1e-12 || back_radius_m > radius_m + 1e-12 {
        return Err("The flat front and back cannot be wider than the body.".into());
    }
    if front_radius_m < 0.0 || back_radius_m < 0.0 {
        return Err("The flat front and back cannot have a negative size.".into());
    }
    if radius_m <= 2.0 * wall_m || length_m <= 3.0 * wall_m {
        return Err("The wall leaves no air in this shape.".into());
    }
    let mut shape = Revolved {
        radius_m,
        length_m,
        front_radius_m,
        back_radius_m,
        widest_at,
        wall_m,
        outer: Vec::new(),
        inner: Vec::new(),
    };
    shape.outer = (0..=PROFILE_POINTS)
        .map(|index| {
            // Even steps in the ellipses' angle, so the steep ends are well sampled.
            let t = index as f64 / PROFILE_POINTS as f64;
            let ym = shape.widest_at * shape.length_m;
            if t <= 0.5 {
                let angle = t * std::f64::consts::PI;
                [
                    ym * (1.0 - angle.cos()),
                    shape.front_radius_m + (shape.radius_m - shape.front_radius_m) * angle.sin(),
                ]
            } else {
                let angle = (t - 0.5) * std::f64::consts::PI;
                [
                    ym + (shape.length_m - ym) * angle.sin(),
                    shape.back_radius_m + (shape.radius_m - shape.back_radius_m) * angle.cos(),
                ]
            }
        })
        .collect();
    // The inside: at each depth, the largest radius still `wall_m` from the outer surface.
    // It spans the depths at which the axis itself is that far inside.
    let on_axis = |y: f64| shape.distance_to_outside(y, 0.0) >= wall_m;
    let middle = shape.widest_at * length_m;
    if !on_axis(middle) {
        return Err("The wall leaves no air in this shape.".into());
    }
    let edge = |mut outside: f64, mut inside: f64| {
        for _ in 0..48 {
            let y = 0.5 * (outside + inside);
            if on_axis(y) {
                inside = y;
            } else {
                outside = y;
            }
        }
        inside
    };
    let (first, last) = (edge(0.0, middle), edge(length_m, middle));
    for index in 0..=PROFILE_POINTS {
        let y = first + (last - first) * index as f64 / PROFILE_POINTS as f64;
        let (mut low, mut high) = (0.0, radius_at_or_end(&shape.outer, y));
        for _ in 0..48 {
            let middle = 0.5 * (low + high);
            if shape.distance_to_outside(y, middle) >= wall_m {
                low = middle;
            } else {
                high = middle;
            }
        }
        shape.inner.push([y, low]);
    }
    if shape.inner.iter().map(|p| p[1]).fold(0.0, f64::max) <= 0.0 {
        return Err("The wall leaves no air in this shape.".into());
    }
    Ok(Box::new(shape))
}

/// Volume of the solid of revolution under a profile `[y, r]`: one cone frustum per step.
fn volume_under(profile: &[[f64; 2]]) -> f64 {
    profile
        .windows(2)
        .map(|pair| {
            let (h, a, b) = (pair[1][0] - pair[0][0], pair[0][1], pair[1][1]);
            std::f64::consts::PI * h * (a * a + a * b + b * b) / 3.0
        })
        .sum()
}

/// The radius of a profile at depth `y`, its ends included.
fn radius_at_or_end(profile: &[[f64; 2]], y: f64) -> f64 {
    let (first, last) = (profile[0], profile[profile.len() - 1]);
    if y <= first[0] {
        first[1]
    } else if y >= last[0] {
        last[1]
    } else {
        radius_at(profile, y)
    }
}

/// The radius of a profile at depth `y` (0 outside its span).
fn radius_at(profile: &[[f64; 2]], y: f64) -> f64 {
    let (Some(first), Some(last)) = (profile.first(), profile.last()) else {
        return 0.0;
    };
    if y <= first[0] || y >= last[0] {
        return 0.0;
    }
    // Depth rises along the profile, so the step holding `y` is found by bisection.
    let (mut low, mut high) = (0, profile.len() - 1);
    while high - low > 1 {
        let middle = (low + high) / 2;
        if profile[middle][0] <= y {
            low = middle;
        } else {
            high = middle;
        }
    }
    let (a, b) = (profile[low], profile[high]);
    if b[0] - a[0] < 1e-15 {
        return a[1].max(b[1]);
    }
    a[1] + (b[1] - a[1]) * (y - a[0]) / (b[0] - a[0])
}

impl Revolved {
    /// Distance from a point inside, at depth `y` and radius `r`, to the outer surface: the
    /// curved skin, the flat front, and the flat back.
    fn distance_to_outside(&self, y: f64, r: f64) -> f64 {
        let mut best = y.min(self.length_m - y);
        for pair in self.outer.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dy, dr) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dy * dy + dr * dr;
            let t = if len2 > 0.0 {
                (((y - a[0]) * dy + (r - a[1]) * dr) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (py, pr) = (a[0] + t * dy, a[1] + t * dr);
            best = best.min(((y - py).powi(2) + (r - pr).powi(2)).sqrt());
        }
        best
    }

    fn radial(&self, p: [f64; 3]) -> f64 {
        ((p[0] - self.radius_m).powi(2) + (p[2] - self.radius_m).powi(2)).sqrt()
    }

    /// A profile closed on the axis, as `[r, y]` corners counter-clockwise, for the `revolve`
    /// solid.
    fn closed(profile: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let mut out = vec![[0.0, profile[0][0]]];
        for point in profile {
            if point[1] > 1e-9 {
                out.push([point[1], point[0]]);
            }
        }
        out.push([0.0, profile[profile.len() - 1][0]]);
        out
    }

    fn revolve(&self, profile: &[[f64; 2]]) -> Value {
        json!({ "op": "revolve", "profile_m": Self::closed(profile),
                "origin_m": [self.radius_m, 0.0, self.radius_m], "segments": 96 })
    }
}

impl OuterShape for Revolved {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(volume_under(&self.inner))
    }

    fn baffle_width_m(&self) -> f64 {
        2.0 * self.radius_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        Ok([2.0 * self.radius_m, self.length_m, 2.0 * self.radius_m])
    }

    fn surfaces(&self) -> Vec<String> {
        vec!["front".into(), "back".into()]
    }

    /// `[u, v]` is measured from the lower-left corner of the square the body fits in, as
    /// seen from outside, so the middle of either flat face is at `[radius, radius]`.
    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let (u, v) = (at_m[0], at_m[1]);
        let off = ((u - self.radius_m).powi(2) + (v - self.radius_m).powi(2)).sqrt();
        let (flat, y, outward, x) = match surface {
            "front" => (self.front_radius_m, 0.0, [0.0, -1.0, 0.0], u),
            "back" => (
                self.back_radius_m,
                self.length_m,
                [0.0, 1.0, 0.0],
                2.0 * self.radius_m - u,
            ),
            other => {
                return Err(format!(
                    "A revolved shape has a flat front and a flat back, not a {other} surface."
                ))
            }
        };
        if off > flat {
            return Err(format!(
                "The mount at ({u:.3}, {v:.3}) m is off the flat {surface}, which is {:.0} mm across.",
                2000.0 * flat
            ));
        }
        Ok(MountPose {
            position_m: [x, y, v],
            outward,
        })
    }

    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        if outward[1] < -0.5 {
            Some("front".into())
        } else if outward[1] > 0.5 {
            Some("back".into())
        } else {
            None
        }
    }

    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        let (flat, y) = match surface {
            "front" => (self.front_radius_m, 0.0),
            "back" => (self.back_radius_m, self.length_m),
            other => return Err(format!("A revolved shape has no {other} surface.")),
        };
        let r = self.radius_m;
        let count = 256;
        Ok((0..count)
            .map(|i| {
                let angle = 2.0 * std::f64::consts::PI * i as f64 / count as f64;
                [r + flat * angle.cos(), y, r + flat * angle.sin()]
            })
            .collect())
    }

    fn contains_air(&self, p: [f64; 3]) -> Result<bool, String> {
        Ok(self.radial(p) < radius_at(&self.inner, p[1]))
    }

    fn contains_cabinet(&self, p: [f64; 3]) -> Result<bool, String> {
        Ok(self.radial(p) < radius_at(&self.outer, p[1]))
    }

    fn contains_solid(&self, p: [f64; 3]) -> Result<bool, String> {
        Ok(self.contains_cabinet(p)? && !self.contains_air(p)?)
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.wall_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, area)| area).sum();
        Ok((volume_under(&self.outer) - volume_under(&self.inner) - area * self.wall_m).max(0.0))
    }

    fn printed_part_sizes_m(&self) -> Vec<[f64; 3]> {
        vec![[2.0 * self.radius_m, self.length_m, 2.0 * self.radius_m]]
    }

    fn notes(&self) -> Vec<String> {
        vec![format!(
            "A curved body: the everyday model takes the rim of the flat {:.0} mm front as its \
             baffle edge, though the skin curves away gently there, so the real edge effect is \
             milder. Run the outside in 3D to see it.",
            2000.0 * self.front_radius_m
        )]
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let mut parts = vec![self.revolve(&self.outer), self.revolve(&self.inner)];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, self.wall_m)),
        );
        Ok(csg_difference(parts))
    }

    fn air_tree(&self) -> Result<Value, String> {
        Ok(self.revolve(&self.inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(fields: Value) -> Box<dyn OuterShape> {
        parse(fields.as_object().unwrap()).unwrap()
    }

    #[test]
    fn a_cylinder_has_the_closed_form_volumes() {
        // Flat ends as wide as the body: a plain cylinder with a wall all round.
        let s = shape(
            json!({ "diameter_m": 0.2, "length_m": 0.3, "front_diameter_m": 0.2,
            "back_diameter_m": 0.2, "wall_m": 0.01 }),
        );
        let inside = std::f64::consts::PI * 0.09f64.powi(2) * 0.28;
        let outside = std::f64::consts::PI * 0.1f64.powi(2) * 0.3;
        assert!((s.interior_volume_m3().unwrap() - inside).abs() < 1e-4 * inside);
        assert!((s.print_solid_m3(&[]).unwrap() - (outside - inside)).abs() < 1e-3 * outside);
    }

    #[test]
    fn a_cut_off_sphere_has_the_closed_form_volume() {
        // Widest in the middle, with no flat ends: a spheroid of half axes 0.1 and 0.15.
        let s = shape(
            json!({ "diameter_m": 0.2, "length_m": 0.3, "front_diameter_m": 0.0,
            "back_diameter_m": 0.0, "widest_at": 0.5, "wall_m": 0.01 }),
        );
        let outside = 4.0 / 3.0 * std::f64::consts::PI * 0.1 * 0.1 * 0.15;
        let solid = s.print_solid_m3(&[]).unwrap();
        let air = s.interior_volume_m3().unwrap();
        assert!(
            (solid + air - outside).abs() < 2e-3 * outside,
            "{}",
            solid + air
        );
        // A sphere's inside at a uniform wall is the smaller sphere.
        let ball = shape(
            json!({ "diameter_m": 0.2, "length_m": 0.2, "front_diameter_m": 0.0,
            "back_diameter_m": 0.0, "widest_at": 0.5, "wall_m": 0.01 }),
        );
        let inner = 4.0 / 3.0 * std::f64::consts::PI * 0.09f64.powi(3);
        let air = ball.interior_volume_m3().unwrap();
        assert!((air - inner).abs() < 3e-3 * inner, "{air} against {inner}");
    }

    #[test]
    fn the_inside_tests_agree_with_the_volumes() {
        let s = shape(
            json!({ "diameter_m": 0.22, "length_m": 0.26, "front_diameter_m": 0.15,
            "back_diameter_m": 0.08, "widest_at": 0.4, "wall_m": 0.008 }),
        );
        // A fixed, even grid of points through the bounding box.
        let n = 120;
        let (mut air, mut body) = (0usize, 0usize);
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    let p = [
                        0.22 * (i as f64 + 0.5) / n as f64,
                        0.26 * (j as f64 + 0.5) / n as f64,
                        0.22 * (k as f64 + 0.5) / n as f64,
                    ];
                    air += s.contains_air(p).unwrap() as usize;
                    body += s.contains_cabinet(p).unwrap() as usize;
                    assert!(!s.contains_air(p).unwrap() || s.contains_cabinet(p).unwrap());
                }
            }
        }
        let cell = 0.22 * 0.26 * 0.22 / (n * n * n) as f64;
        let formula_air = s.interior_volume_m3().unwrap();
        let formula_body = formula_air + s.print_solid_m3(&[]).unwrap();
        assert!((air as f64 * cell - formula_air).abs() < 0.01 * formula_air);
        assert!((body as f64 * cell - formula_body).abs() < 0.01 * formula_body);
    }

    #[test]
    fn a_driver_sits_on_the_flat_front_only() {
        let s = shape(
            json!({ "diameter_m": 0.22, "length_m": 0.26, "front_diameter_m": 0.15,
            "back_diameter_m": 0.08, "widest_at": 0.4, "wall_m": 0.008 }),
        );
        let pose = s.mount_pose("front", [0.11, 0.11]).unwrap();
        assert_eq!(pose.position_m, [0.11, 0.0, 0.11]);
        assert_eq!(pose.outward, [0.0, -1.0, 0.0]);
        assert!(s.mount_pose("front", [0.11, 0.2]).is_err());
        assert!(s.mount_pose("top", [0.11, 0.11]).is_err());
        assert_eq!(
            s.mount_pose("back", [0.11, 0.11]).unwrap().position_m[1],
            0.26
        );
        assert_eq!(s.surface_outline("front").unwrap().len(), 256);
    }
}
