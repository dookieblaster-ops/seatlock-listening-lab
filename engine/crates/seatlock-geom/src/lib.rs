//! Cabinet geometry: air network, solid primitives, voxel grid, and volumes.

pub mod edged;
pub mod prism;
pub mod revolved;

pub use seatlock_core::registry::MountPose;
use seatlock_core::registry::{OuterShape, Registry};
use serde_json::{json, Map, Value};

pub struct BoxShape {
    pub width_m: f64,
    pub height_m: f64,
    pub depth_m: f64,
    pub wall_m: f64,
}

impl OuterShape for BoxShape {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        let width = self.width_m - 2.0 * self.wall_m;
        let height = self.height_m - 2.0 * self.wall_m;
        let depth = self.depth_m - 2.0 * self.wall_m;
        if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
            return Err("The walls are thicker than the box.".to_string());
        }
        Ok(width * height * depth)
    }

    fn baffle_width_m(&self) -> f64 {
        self.width_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        Ok([self.width_m, self.depth_m, self.height_m])
    }

    fn surfaces(&self) -> Vec<String> {
        ["front", "back", "left", "right", "bottom", "top"]
            .iter()
            .map(|name| name.to_string())
            .collect()
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        BoxShape::mount_pose(self, surface, at_m)
    }

    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        Some(box_face_facing(outward).to_string())
    }

    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        let (w, d, h) = (self.width_m, self.depth_m, self.height_m);
        Ok(match surface {
            "front" => vec![[0.0, 0.0, 0.0], [w, 0.0, 0.0], [w, 0.0, h], [0.0, 0.0, h]],
            "back" => vec![[w, d, 0.0], [0.0, d, 0.0], [0.0, d, h], [w, d, h]],
            "left" => vec![[0.0, d, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, h], [0.0, d, h]],
            "right" => vec![[w, 0.0, 0.0], [w, d, 0.0], [w, d, h], [w, 0.0, h]],
            "bottom" => vec![[0.0, d, 0.0], [w, d, 0.0], [w, 0.0, 0.0], [0.0, 0.0, 0.0]],
            "top" => vec![[0.0, 0.0, h], [w, 0.0, h], [w, d, h], [0.0, d, h]],
            other => return Err(format!("unknown face {other}")),
        })
    }

    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        Ok(BoxShape::contains_air(self, point_m))
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.wall_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, area)| area).sum();
        self.solid_volume_m3(area)
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let w = self.wall_m;
        let mut parts = vec![
            csg_box([0.0, 0.0, 0.0], [self.width_m, self.depth_m, self.height_m]),
            csg_box(
                [w, w, w],
                [self.width_m - w, self.depth_m - w, self.height_m - w],
            ),
        ];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, w)),
        );
        Ok(csg_difference(parts))
    }

    fn contains_solid(&self, p: [f64; 3]) -> Result<bool, String> {
        let outside = p[0] > 0.0
            && p[0] < self.width_m
            && p[1] > 0.0
            && p[1] < self.depth_m
            && p[2] > 0.0
            && p[2] < self.height_m;
        Ok(outside && !BoxShape::contains_air(self, p))
    }

    fn contains_cabinet(&self, p: [f64; 3]) -> Result<bool, String> {
        Ok(p[0] > 0.0
            && p[0] < self.width_m
            && p[1] > 0.0
            && p[1] < self.depth_m
            && p[2] > 0.0
            && p[2] < self.height_m)
    }

    fn air_tree(&self) -> Result<Value, String> {
        let w = self.wall_m;
        Ok(csg_box(
            [w, w, w],
            [self.width_m - w, self.depth_m - w, self.height_m - w],
        ))
    }
}

/// The box face whose outward direction is closest to `outward`.
pub fn box_face_facing(outward: [f64; 3]) -> &'static str {
    let [x, y, z] = outward;
    if y.abs() >= x.abs() && y.abs() >= z.abs() {
        if y < 0.0 {
            "front"
        } else {
            "back"
        }
    } else if x.abs() >= z.abs() {
        if x < 0.0 {
            "left"
        } else {
            "right"
        }
    } else if z < 0.0 {
        "bottom"
    } else {
        "top"
    }
}

/// A box from `min` to `max` in m.
pub fn csg_box(min: [f64; 3], max: [f64; 3]) -> Value {
    json!({ "op": "box", "min_m": min, "max_m": max })
}

/// A cylinder whose base circle is centred on `base`, extending `length_m` along `axis`.
pub fn csg_cylinder(base: [f64; 3], axis: [f64; 3], length_m: f64, radius_m: f64) -> Value {
    json!({ "op": "cylinder", "base_m": base, "axis": axis, "length_m": length_m,
            "radius_m": radius_m, "segments": 96 })
}

/// The first part minus all the others.
pub fn csg_difference(parts: Vec<Value>) -> Value {
    json!({ "op": "difference", "of": parts })
}

pub fn csg_union(parts: Vec<Value>) -> Value {
    json!({ "op": "union", "of": parts })
}

/// A cut-out through a panel of `thickness_m` at a mount, a little longer on both sides.
pub fn csg_hole(pose: &MountPose, diameter_m: f64, thickness_m: f64) -> Value {
    let extra = 0.0005;
    let o = pose.outward;
    let base = [
        pose.position_m[0] + o[0] * extra,
        pose.position_m[1] + o[1] * extra,
        pose.position_m[2] + o[2] * extra,
    ];
    csg_cylinder(
        base,
        [-o[0], -o[1], -o[2]],
        thickness_m + 2.0 * extra,
        diameter_m / 2.0,
    )
}

/// Air volume as the voxel grid sees it: each `dx` cube counts the fraction of its
/// 4 x 4 x 4 sample points that are in the air (README section 6.3, CHK-GEO-1).
pub fn voxel_air_volume_m3(shape: &dyn OuterShape, dx: f64) -> Result<f64, String> {
    voxel_air_volume_where(shape, dx, &|_| true)
}

/// As `voxel_air_volume_m3`, counting only air where `also` holds, such as inside a chamber.
pub fn voxel_air_volume_where(
    shape: &dyn OuterShape,
    dx: f64,
    also: &dyn Fn([f64; 3]) -> bool,
) -> Result<f64, String> {
    let [sx, sy, sz] = shape.outer_size_m()?;
    let cells = |size: f64| (size / dx).ceil().max(1.0) as usize;
    let (nx, ny, nz) = (cells(sx), cells(sy), cells(sz));
    let n = 4;
    let step = dx / n as f64;
    let mut volume = 0.0;
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let mut inside = 0;
                for a in 0..n {
                    for b in 0..n {
                        for c in 0..n {
                            let point = [
                                i as f64 * dx + (a as f64 + 0.5) * step,
                                j as f64 * dx + (b as f64 + 0.5) * step,
                                k as f64 * dx + (c as f64 + 0.5) * step,
                            ];
                            if shape.contains_air(point)? && also(point) {
                                inside += 1;
                            }
                        }
                    }
                }
                volume += dx * dx * dx * inside as f64 / (n * n * n) as f64;
            }
        }
    }
    Ok(volume)
}

impl BoxShape {
    pub fn outer_volume_m3(&self) -> f64 {
        self.width_m * self.height_m * self.depth_m
    }

    pub fn solid_volume_m3(&self, cutout_area_m2: f64) -> Result<f64, String> {
        let air = self.interior_volume_m3()?;
        Ok((self.outer_volume_m3() - air - cutout_area_m2 * self.wall_m).max(0.0))
    }

    pub fn contains_air(&self, point: [f64; 3]) -> bool {
        let wall = self.wall_m;
        point[0] > wall
            && point[0] < self.width_m - wall
            && point[1] > wall
            && point[1] < self.depth_m - wall
            && point[2] > wall
            && point[2] < self.height_m - wall
    }

    /// Sum of the parts of  `dx` cubes that lie in the interior air.
    pub fn voxel_air_volume_m3(&self, dx: f64) -> Result<f64, String> {
        let air = self.interior_volume_m3()?;
        let wall = self.wall_m;
        let x0 = wall;
        let y0 = wall;
        let z0 = wall;
        let x1 = self.width_m - wall;
        let y1 = self.depth_m - wall;
        let z1 = self.height_m - wall;
        let mut volume = 0.0;
        let mut x = (x0 / dx).floor() * dx;
        while x < x1 {
            let mut y = (y0 / dx).floor() * dx;
            while y < y1 {
                let mut z = (z0 / dx).floor() * dx;
                while z < z1 {
                    let overlap_x = (x + dx).min(x1) - x.max(x0);
                    let overlap_y = (y + dx).min(y1) - y.max(y0);
                    let overlap_z = (z + dx).min(z1) - z.max(z0);
                    if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
                        volume += overlap_x * overlap_y * overlap_z;
                    }
                    z += dx;
                }
                y += dx;
            }
            x += dx;
        }
        let _ = air;
        Ok(volume)
    }

    /// `[u, v]` is from the face's lower-left corner as seen from outside.
    pub fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let (u, v) = (at_m[0], at_m[1]);
        let pose = match surface {
            "front" => MountPose {
                position_m: [u, 0.0, v],
                outward: [0.0, -1.0, 0.0],
            },
            "back" => MountPose {
                position_m: [self.width_m - u, self.depth_m, v],
                outward: [0.0, 1.0, 0.0],
            },
            "left" => MountPose {
                position_m: [0.0, self.depth_m - u, v],
                outward: [-1.0, 0.0, 0.0],
            },
            "right" => MountPose {
                position_m: [self.width_m, u, v],
                outward: [1.0, 0.0, 0.0],
            },
            "bottom" => MountPose {
                position_m: [u, v, 0.0],
                outward: [0.0, 0.0, -1.0],
            },
            "top" => MountPose {
                position_m: [u, self.depth_m - v, self.height_m],
                outward: [0.0, 0.0, 1.0],
            },
            other => return Err(format!("unknown face {other}")),
        };
        Ok(pose)
    }
}

pub fn register(registry: &mut Registry) {
    registry.register_outer("box", parse_box);
    registry.register_outer("rounded", parse_rounded);
    registry.register_outer("chamfered", parse_chamfered);
    registry.register_outer("sloped", parse_sloped);
    registry.register_outer("prism", parse_prism);
    registry.register_outer("tube", parse_tube);
    registry.register_outer("sheet_box", parse_sheet_box);
    registry.register_outer("revolved", revolved::parse);
}

/// A box cut from sheet stock (README section 6.2): butt-jointed panels, nothing printed.
/// The front and back are full size; the sides fit between them; the top and bottom fit
/// between the sides. The wall is the sheet's thickness.
pub struct SheetBox {
    pub shape: BoxShape,
    pub stock_id: String,
    pub stock_name: String,
}

impl OuterShape for SheetBox {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        self.shape.interior_volume_m3()
    }
    fn baffle_width_m(&self) -> f64 {
        self.shape.width_m
    }
    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        OuterShape::outer_size_m(&self.shape)
    }
    fn surfaces(&self) -> Vec<String> {
        OuterShape::surfaces(&self.shape)
    }
    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        self.shape.mount_pose(surface, at_m)
    }
    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        Some(box_face_facing(outward).to_string())
    }
    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        OuterShape::surface_outline(&self.shape, surface)
    }
    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        Ok(self.shape.contains_air(point_m))
    }
    fn contains_cabinet(&self, point_m: [f64; 3]) -> Result<bool, String> {
        OuterShape::contains_cabinet(&self.shape, point_m)
    }
    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.shape.wall_m)
    }
    fn print_solid_m3(&self, _cutouts: &[(String, f64)]) -> Result<f64, String> {
        Ok(0.0)
    }
    fn printed_part_sizes_m(&self) -> Vec<[f64; 3]> {
        Vec::new()
    }
    fn stock_panels(&self) -> Vec<(String, String, [f64; 2])> {
        let BoxShape {
            width_m: w,
            height_m: h,
            depth_m: d,
            wall_m: t,
        } = self.shape;
        let id = &self.stock_id;
        [
            ("front", [w, h]),
            ("back", [w, h]),
            ("left side", [d - 2.0 * t, h]),
            ("right side", [d - 2.0 * t, h]),
            ("top", [w - 2.0 * t, d - 2.0 * t]),
            ("bottom", [w - 2.0 * t, d - 2.0 * t]),
        ]
        .into_iter()
        .map(|(name, size)| (id.clone(), name.to_string(), size))
        .collect()
    }
    fn notes(&self) -> Vec<String> {
        vec![format!(
            "Cut from {}: nothing is printed. Driver holes are cut with a hole saw or jigsaw.",
            self.stock_name
        )]
    }
    fn print_tree(&self, _cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        Ok(csg_union(Vec::new()))
    }
}

fn parse_sheet_box(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let stock_id = fields.get("stock_id").and_then(Value::as_str).unwrap_or("");
    let record = seatlock_core::catalog::stock(stock_id)
        .ok_or_else(|| format!("Sheet stock {stock_id} is not in the catalog."))?;
    let wall_m = record
        .get("thickness_m")
        .and_then(Value::as_f64)
        .ok_or("The sheet's record has no thickness.")?;
    let name = record
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(stock_id);
    Ok(Box::new(SheetBox {
        shape: BoxShape {
            width_m: required_f64(fields, "width_m")?,
            height_m: required_f64(fields, "height_m")?,
            depth_m: required_f64(fields, "depth_m")?,
            wall_m,
        },
        stock_id: stock_id.to_string(),
        stock_name: name.to_string(),
    }))
}

pub struct TubeShape {
    pub inside_diameter_m: f64,
    pub outside_diameter_m: f64,
    /// Length of the stock cut. The printed end caps sit outside it.
    pub length_m: f64,
    pub wall_from_user: bool,
    /// Thickness of each printed end cap: a flat disc of the tube's outside diameter.
    pub cap_thickness_m: f64,
    /// The stock record the tube is cut from.
    pub stock_id: String,
}

/// End caps are 8 mm thick unless the design says otherwise.
pub const DEFAULT_CAP_THICKNESS_M: f64 = 0.008;

impl TubeShape {
    /// Overall length: the cut plus both caps. The front cap's outer face is at y = 0.
    pub fn overall_length_m(&self) -> f64 {
        self.length_m + 2.0 * self.cap_thickness_m
    }

    /// Mounts sit on the end caps. Surface coordinates are `[u, v]` from the lower-left corner
    /// of the square around the cap, seen from outside, as on a flat box face.
    pub fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let d = self.outside_diameter_m;
        let (u, v) = (at_m[0], at_m[1]);
        let r = ((u - d / 2.0).powi(2) + (v - d / 2.0).powi(2)).sqrt();
        if r > d / 2.0 {
            return Err(format!(
                "The mount at ({u:.3}, {v:.3}) m is outside the {:.0} mm end cap.",
                d * 1000.0
            ));
        }
        match surface {
            "front" => Ok(MountPose {
                position_m: [u, 0.0, v],
                outward: [0.0, -1.0, 0.0],
            }),
            "back" => Ok(MountPose {
                position_m: [d - u, self.overall_length_m(), v],
                outward: [0.0, 1.0, 0.0],
            }),
            other => Err(format!(
                "A tube has a front and a back end cap, not a {other} surface."
            )),
        }
    }

    /// Volume of both printed caps, before any cut-outs.
    pub fn cap_volume_m3(&self) -> f64 {
        2.0 * std::f64::consts::PI * (self.outside_diameter_m / 2.0).powi(2) * self.cap_thickness_m
    }
}

impl OuterShape for TubeShape {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        let radius = self.inside_diameter_m / 2.0;
        if radius <= 0.0 || self.length_m <= 0.0 {
            return Err("The tube needs an inside diameter and a length.".into());
        }
        Ok(std::f64::consts::PI * radius * radius * self.length_m)
    }
    fn baffle_width_m(&self) -> f64 {
        self.outside_diameter_m
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        let d = self.outside_diameter_m;
        Ok([d, self.overall_length_m(), d])
    }

    fn surfaces(&self) -> Vec<String> {
        vec!["front".into(), "back".into()]
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        TubeShape::mount_pose(self, surface, at_m)
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
        let y = match surface {
            "front" => 0.0,
            "back" => self.overall_length_m(),
            other => return Err(format!("A tube has no {other} surface.")),
        };
        let r = self.outside_diameter_m / 2.0;
        let count = 256;
        Ok((0..count)
            .map(|i| {
                let angle = 2.0 * std::f64::consts::PI * i as f64 / count as f64;
                [r + r * angle.cos(), y, r + r * angle.sin()]
            })
            .collect())
    }

    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        let r = self.outside_diameter_m / 2.0;
        let inner = self.inside_diameter_m / 2.0;
        let radial = ((point_m[0] - r).powi(2) + (point_m[2] - r).powi(2)).sqrt();
        let y = point_m[1];
        Ok(radial < inner && y > self.cap_thickness_m && y < self.cap_thickness_m + self.length_m)
    }

    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        Ok(self.cap_thickness_m)
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let area: f64 = cutouts.iter().map(|(_, area)| area).sum();
        Ok((self.cap_volume_m3() - area * self.cap_thickness_m).max(0.0))
    }

    fn stock_cuts(&self) -> Vec<(String, f64)> {
        vec![(self.stock_id.clone(), self.length_m)]
    }

    fn printed_part_sizes_m(&self) -> Vec<[f64; 3]> {
        let d = self.outside_diameter_m;
        vec![[d, self.cap_thickness_m, d]; 2]
    }

    fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.wall_from_user {
            notes.push(
                "The seller does not print a wall thickness, so this wall is the user's own number."
                    .into(),
            );
        }
        notes.push(format!(
            "Two printed end caps, {:.1} mm thick. The pipe is a stock cut, not a print.",
            self.cap_thickness_m * 1000.0
        ));
        notes
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let r = self.outside_diameter_m / 2.0;
        let t = self.cap_thickness_m;
        let axis = [0.0, 1.0, 0.0];
        let caps = csg_union(vec![
            csg_cylinder([r, 0.0, r], axis, t, r),
            csg_cylinder([r, t + self.length_m, r], axis, t, r),
        ]);
        let mut parts = vec![caps];
        parts.extend(
            cutouts
                .iter()
                .map(|(pose, diameter)| csg_hole(pose, *diameter, t)),
        );
        Ok(csg_difference(parts))
    }

    fn contains_solid(&self, p: [f64; 3]) -> Result<bool, String> {
        let r = self.outside_diameter_m / 2.0;
        let radial = ((p[0] - r).powi(2) + (p[2] - r).powi(2)).sqrt();
        let t = self.cap_thickness_m;
        let end = self.overall_length_m();
        Ok(radial < r && ((p[1] > 0.0 && p[1] < t) || (p[1] > end - t && p[1] < end)))
    }

    fn contains_cabinet(&self, p: [f64; 3]) -> Result<bool, String> {
        let r = self.outside_diameter_m / 2.0;
        let radial = ((p[0] - r).powi(2) + (p[2] - r).powi(2)).sqrt();
        Ok(radial < r && p[1] > 0.0 && p[1] < self.overall_length_m())
    }

    fn air_tree(&self) -> Result<Value, String> {
        let r = self.outside_diameter_m / 2.0;
        Ok(csg_cylinder(
            [r, self.cap_thickness_m, r],
            [0.0, 1.0, 0.0],
            self.length_m,
            self.inside_diameter_m / 2.0,
        ))
    }
}

fn parse_box(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(BoxShape {
        width_m: required_f64(fields, "width_m")?,
        height_m: required_f64(fields, "height_m")?,
        depth_m: required_f64(fields, "depth_m")?,
        wall_m: required_f64(fields, "wall_m")?,
    }))
}

fn parse_rounded(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let edge = edged::Edge::Round(required_f64(fields, "edge_radius_m")?);
    Ok(Box::new(edged::EdgedBox::new(box_from(fields)?, edge)?))
}

fn parse_chamfered(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let edge = edged::Edge::Chamfer(required_f64(fields, "chamfer_m")?);
    Ok(Box::new(edged::EdgedBox::new(box_from(fields)?, edge)?))
}

fn parse_sloped(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(prism::sloped(
        required_f64(fields, "width_m")?,
        required_f64(fields, "height_m")?,
        required_f64(fields, "depth_bottom_m")?,
        required_f64(fields, "depth_top_m")?,
        required_f64(fields, "wall_m")?,
    )?))
}

/// `outline_m` is the front, `[x, z]` corners in order, extruded `length_m` back along y.
fn parse_prism(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let outline: Vec<[f64; 2]> = fields
        .get("outline_m")
        .and_then(Value::as_array)
        .ok_or("missing outline_m")?
        .iter()
        .map(|point| {
            let (a, b) = point2(point);
            [a, b]
        })
        .collect();
    Ok(Box::new(prism::Prism::new(
        prism::Axis::Y,
        &outline,
        required_f64(fields, "length_m")?,
        required_f64(fields, "wall_m")?,
        ["front", "back"],
        None,
    )?))
}

pub fn describe_tube(fields: &Map<String, Value>) -> Result<TubeShape, String> {
    let length_m = required_f64(fields, "length_m")?;
    let stock_id = fields.get("stock_id").and_then(Value::as_str).unwrap_or("");
    let record = seatlock_core::catalog::stock(stock_id);
    let inside = record
        .as_ref()
        .and_then(|item| item.get("inside_diameter_m").and_then(Value::as_f64));
    let outside = record
        .as_ref()
        .and_then(|item| item.get("outside_diameter_m").and_then(Value::as_f64));
    let (inside_diameter_m, outside_diameter_m, wall_from_user) = if let Some(inside) = inside {
        let outside = outside.unwrap_or(inside + 2.0 * optional_f64(fields, "wall_m"));
        (inside, outside, false)
    } else if let Some(outside) = outside {
        let wall = optional_f64(fields, "wall_m");
        if wall <= 0.0 {
            return Err(
                "This tube's page does not print a wall, so the design must give wall_m.".into(),
            );
        }
        (outside - 2.0 * wall, outside, true)
    } else {
        let diameter = required_f64(fields, "inside_diameter_m")?;
        (diameter, diameter, false)
    };
    let cap_thickness_m = fields
        .get("cap_thickness_m")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_CAP_THICKNESS_M);
    Ok(TubeShape {
        inside_diameter_m,
        outside_diameter_m,
        length_m,
        wall_from_user,
        cap_thickness_m,
        stock_id: stock_id.to_string(),
    })
}

fn parse_tube(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(describe_tube(fields)?))
}

fn box_from(fields: &Map<String, Value>) -> Result<BoxShape, String> {
    Ok(BoxShape {
        width_m: required_f64(fields, "width_m")?,
        height_m: required_f64(fields, "height_m")?,
        depth_m: required_f64(fields, "depth_m")?,
        wall_m: required_f64(fields, "wall_m")?,
    })
}

fn point2(value: &Value) -> (f64, f64) {
    let coord = |index: usize| {
        value
            .as_array()
            .and_then(|point| point.get(index))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    (coord(0), coord(1))
}

fn optional_f64(fields: &Map<String, Value>, key: &str) -> f64 {
    fields.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn required_f64(fields: &Map<String, Value>, key: &str) -> Result<f64, String> {
    fields
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing {key}"))
}
