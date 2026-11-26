//! The outer shape `loft` (README section 6.2): rectangular cross-sections placed along a
//! front-to-back spine, linearly interpolated between stations.
//!
//! ```text
//! { "kind": "loft", "depth_m": 0.22, "wall_m": 0.008,
//!   "sections": [
//!     { "at": 0.0, "width_m": 0.18, "height_m": 0.28 },
//!     { "at": 0.4, "width_m": 0.22, "height_m": 0.32 },
//!     { "at": 1.0, "width_m": 0.14, "height_m": 0.24 }
//!   ] }
//! ```
//!
//! `at` is the fraction of `depth_m` from the front (0) to the back (1). Each section is
//! centred in the bounding box so a taper stays aligned on the listening axis. Surfaces are
//! `front` and `back` (the end rectangles); mounts must sit on those flats.

use seatlock_core::registry::{MountPose, OuterShape};
use serde_json::{json, Map, Value};

use crate::{csg_difference, csg_hole, csg_union};

#[derive(Clone, Copy, Debug)]
struct Section {
    /// Depth along +y, metres.
    y: f64,
    width_m: f64,
    height_m: f64,
}

pub struct Loft {
    depth_m: f64,
    wall_m: f64,
    /// Bounding width and height (max over sections).
    bound_w: f64,
    bound_h: f64,
    outer: Vec<Section>,
    inner: Vec<Section>,
}

fn number(fields: &Map<String, Value>, key: &str) -> Result<f64, String> {
    fields
        .get(key)
        .and_then(Value::as_f64)
        .ok_or(format!("missing {key}"))
}

pub fn parse(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let depth_m = number(fields, "depth_m")?;
    let wall_m = number(fields, "wall_m")?;
    if !(depth_m > 0.0 && wall_m > 0.0) {
        return Err("A loft needs a depth and a wall.".into());
    }
    if depth_m <= 3.0 * wall_m {
        return Err("The wall leaves no air in this loft.".into());
    }
    let raw = fields
        .get("sections")
        .and_then(Value::as_array)
        .ok_or("A loft needs a sections list.")?;
    if raw.len() < 2 {
        return Err("A loft needs at least two cross-sections along its spine.".into());
    }
    let mut outer = Vec::with_capacity(raw.len());
    for (index, entry) in raw.iter().enumerate() {
        let map = entry
            .as_object()
            .ok_or(format!("loft section {index} is not an object."))?;
        let at = map
            .get("at")
            .and_then(Value::as_f64)
            .ok_or(format!("loft section {index} needs at (0 to 1)."))?;
        if !(0.0..=1.0).contains(&at) {
            return Err(format!("loft section {index}: at must be between 0 and 1."));
        }
        let width_m = map
            .get("width_m")
            .and_then(Value::as_f64)
            .ok_or(format!("loft section {index} needs width_m."))?;
        let height_m = map
            .get("height_m")
            .and_then(Value::as_f64)
            .ok_or(format!("loft section {index} needs height_m."))?;
        if !(width_m > 2.0 * wall_m && height_m > 2.0 * wall_m) {
            return Err(format!(
                "loft section {index}: the wall leaves no air in that cross-section."
            ));
        }
        outer.push(Section {
            y: at * depth_m,
            width_m,
            height_m,
        });
    }
    outer.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    for pair in outer.windows(2) {
        if (pair[1].y - pair[0].y).abs() < 1e-9 {
            return Err("Two loft sections sit at the same place on the spine.".into());
        }
    }
    if (outer[0].y).abs() > 1e-9 || (outer[outer.len() - 1].y - depth_m).abs() > 1e-9 {
        return Err("The first loft section must be at 0 and the last at 1.".into());
    }
    let bound_w = outer.iter().map(|s| s.width_m).fold(0.0, f64::max);
    let bound_h = outer.iter().map(|s| s.height_m).fold(0.0, f64::max);
    // Inside: same stations, shrunk by the wall; front and back faces move inward by wall_m.
    let mut inner = Vec::with_capacity(outer.len());
    for section in &outer {
        let y = section.y.clamp(wall_m, depth_m - wall_m);
        inner.push(Section {
            y,
            width_m: (section.width_m - 2.0 * wall_m).max(0.0),
            height_m: (section.height_m - 2.0 * wall_m).max(0.0),
        });
    }
    // Collapse duplicate depths after clamping the ends.
    inner.dedup_by(|a, b| (a.y - b.y).abs() < 1e-9);
    if inner.len() < 2 || inner.iter().any(|s| s.width_m <= 0.0 || s.height_m <= 0.0) {
        return Err("The wall leaves no air in this loft.".into());
    }
    Ok(Box::new(Loft {
        depth_m,
        wall_m,
        bound_w,
        bound_h,
        outer,
        inner,
    }))
}

impl Loft {
    fn size_at(sections: &[Section], y: f64) -> (f64, f64) {
        let (first, last) = (sections[0], sections[sections.len() - 1]);
        if y <= first.y {
            return (first.width_m, first.height_m);
        }
        if y >= last.y {
            return (last.width_m, last.height_m);
        }
        let (mut low, mut high) = (0, sections.len() - 1);
        while high - low > 1 {
            let middle = (low + high) / 2;
            if sections[middle].y <= y {
                low = middle;
            } else {
                high = middle;
            }
        }
        let (a, b) = (sections[low], sections[high]);
        let t = (y - a.y) / (b.y - a.y).max(1e-15);
        (
            a.width_m + (b.width_m - a.width_m) * t,
            a.height_m + (b.height_m - a.height_m) * t,
        )
    }

    fn rect_in_bound(&self, width: f64, height: f64, y: f64) -> [[f64; 3]; 4] {
        let x0 = (self.bound_w - width) * 0.5;
        let z0 = (self.bound_h - height) * 0.5;
        [
            [x0, y, z0],
            [x0 + width, y, z0],
            [x0 + width, y, z0 + height],
            [x0, y, z0 + height],
        ]
    }

    fn in_rect(&self, p: [f64; 3], width: f64, height: f64) -> bool {
        let x0 = (self.bound_w - width) * 0.5;
        let z0 = (self.bound_h - height) * 0.5;
        p[0] >= x0 && p[0] <= x0 + width && p[2] >= z0 && p[2] <= z0 + height
    }

    fn volume_of(sections: &[Section]) -> f64 {
        sections
            .windows(2)
            .map(|pair| {
                let (a, b) = (pair[0], pair[1]);
                let h = b.y - a.y;
                // Linear interpolation of both dimensions: average of the four corner areas
                // of the frustum of a rectangular pyramid (exact for linear sides).
                let areas = [
                    a.width_m * a.height_m,
                    a.width_m * b.height_m,
                    b.width_m * a.height_m,
                    b.width_m * b.height_m,
                ];
                h * areas.iter().sum::<f64>() / 4.0
            })
            .sum()
    }

    fn hull_between(&self, a: Section, b: Section) -> Value {
        let mut points = Vec::with_capacity(8);
        for corner in self.rect_in_bound(a.width_m, a.height_m, a.y) {
            points.push(corner.to_vec());
        }
        for corner in self.rect_in_bound(b.width_m, b.height_m, b.y) {
            points.push(corner.to_vec());
        }
        json!({ "op": "hull", "points_m": points })
    }

    fn solid_of(&self, sections: &[Section]) -> Value {
        let parts: Vec<Value> = sections
            .windows(2)
            .map(|pair| self.hull_between(pair[0], pair[1]))
            .collect();
        if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            csg_union(parts)
        }
    }
}

impl OuterShape for Loft {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(Self::volume_of(&self.inner))
    }

    fn baffle_width_m(&self) -> f64 {
        self.outer[0].width_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        Ok([self.bound_w, self.depth_m, self.bound_h])
    }

    fn surfaces(&self) -> Vec<String> {
        vec!["front".into(), "back".into()]
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let (u, v) = (at_m[0], at_m[1]);
        let (section, y, outward) = match surface {
            "front" => (self.outer[0], 0.0, [0.0, -1.0, 0.0]),
            "back" => (
                self.outer[self.outer.len() - 1],
                self.depth_m,
                [0.0, 1.0, 0.0],
            ),
            other => {
                return Err(format!(
                    "A loft has a flat front and a flat back, not a {other} surface."
                ))
            }
        };
        // `[u, v]` from the lower-left of the bounding face as seen from outside (README 6.2).
        let x0 = (self.bound_w - section.width_m) * 0.5;
        let z0 = (self.bound_h - section.height_m) * 0.5;
        if u < x0 || u > x0 + section.width_m || v < z0 || v > z0 + section.height_m {
            return Err(format!(
                "The mount at ({u:.3}, {v:.3}) m is off the flat {surface} \
                 ({:.0} x {:.0} mm).",
                section.width_m * 1000.0,
                section.height_m * 1000.0
            ));
        }
        let x = if surface == "back" {
            self.bound_w - u
        } else {
            u
        };
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
        let (section, y) = match surface {
            "front" => (self.outer[0], 0.0),
            "back" => (self.outer[self.outer.len() - 1], self.depth_m),
            other => return Err(format!("A loft has no {other} surface.")),
        };
        Ok(self
            .rect_in_bound(section.width_m, section.height_m, y)
            .to_vec())
    }

    fn contains_air(&self, p: [f64; 3]) -> Result<bool, String> {
        if p[1] < self.inner[0].y || p[1] > self.inner[self.inner.len() - 1].y {
            return Ok(false);
        }
        let (w, h) = Self::size_at(&self.inner, p[1]);
        Ok(self.in_rect(p, w, h))
    }

    fn contains_cabinet(&self, p: [f64; 3]) -> Result<bool, String> {
        if p[1] < 0.0 || p[1] > self.depth_m {
            return Ok(false);
        }
        let (w, h) = Self::size_at(&self.outer, p[1]);
        Ok(self.in_rect(p, w, h))
    }

    fn contains_solid(&self, p: [f64; 3]) -> Result<bool, String> {
        Ok(self.contains_cabinet(p)? && !self.contains_air(p)?)
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.wall_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, a)| a).sum();
        Ok(
            (Self::volume_of(&self.outer) - Self::volume_of(&self.inner) - area * self.wall_m)
                .max(0.0),
        )
    }

    fn printed_part_sizes_m(&self) -> Vec<[f64; 3]> {
        vec![[self.bound_w, self.depth_m, self.bound_h]]
    }

    fn notes(&self) -> Vec<String> {
        vec![format!(
            "A loft of {} rectangular cross-sections along its depth: the everyday model takes \
             the flat front's outline as the baffle edge.",
            self.outer.len()
        )]
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let mut parts = vec![self.solid_of(&self.outer), self.solid_of(&self.inner)];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, self.wall_m)),
        );
        Ok(csg_difference(parts))
    }

    fn air_tree(&self) -> Result<Value, String> {
        Ok(self.solid_of(&self.inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(fields: Value) -> Box<dyn OuterShape> {
        parse(fields.as_object().unwrap()).unwrap()
    }

    #[test]
    fn a_constant_loft_matches_a_box() {
        let s = shape(json!({
            "depth_m": 0.2,
            "wall_m": 0.01,
            "sections": [
                { "at": 0.0, "width_m": 0.16, "height_m": 0.24 },
                { "at": 1.0, "width_m": 0.16, "height_m": 0.24 }
            ]
        }));
        let air = (0.16 - 0.02) * (0.24 - 0.02) * (0.2 - 0.02);
        assert!((s.interior_volume_m3().unwrap() - air).abs() < 1e-12);
        assert_eq!(s.outer_size_m().unwrap(), [0.16, 0.2, 0.24]);
        let pose = s.mount_pose("front", [0.08, 0.12]).unwrap();
        assert_eq!(pose.position_m, [0.08, 0.0, 0.12]);
    }

    #[test]
    fn a_taper_has_air_only_inside_the_shrunk_sections() {
        let s = shape(json!({
            "depth_m": 0.2,
            "wall_m": 0.008,
            "sections": [
                { "at": 0.0, "width_m": 0.2, "height_m": 0.3 },
                { "at": 1.0, "width_m": 0.12, "height_m": 0.18 }
            ]
        }));
        assert!(s.contains_air([0.1, 0.1, 0.15]).unwrap());
        assert!(!s.contains_air([0.01, 0.19, 0.01]).unwrap());
        assert!(s.contains_cabinet([0.1, 0.1, 0.15]).unwrap());
        assert!(s.print_tree(&[]).is_ok());
        assert!(s.interior_volume_m3().unwrap() > 0.0);
    }
}
