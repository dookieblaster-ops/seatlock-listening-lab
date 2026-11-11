//! Cabinet geometry: air network, solid primitives, voxel grid, and volumes.

use seatlock_core::registry::{OuterShape, Registry};
use serde_json::{Map, Value};

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
}

#[derive(Clone, Copy, Debug)]
pub struct MountPose {
    pub position_m: [f64; 3],
    pub outward: [f64; 3],
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
}

struct RoundedBox {
    inner: BoxShape,
    #[allow(dead_code)]
    radius_m: f64,
}

impl OuterShape for RoundedBox {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        self.inner.interior_volume_m3()
    }
    fn baffle_width_m(&self) -> f64 {
        self.inner.baffle_width_m()
    }
}

struct SlopedBox {
    width_m: f64,
    height_m: f64,
    depth_front_m: f64,
    depth_back_m: f64,
    wall_m: f64,
}

impl OuterShape for SlopedBox {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        let width = self.width_m - 2.0 * self.wall_m;
        let height = self.height_m - 2.0 * self.wall_m;
        let depth = 0.5 * (self.depth_front_m + self.depth_back_m) - 2.0 * self.wall_m;
        if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
            return Err("The walls are thicker than the sloped box.".into());
        }
        Ok(width * height * depth)
    }
    fn baffle_width_m(&self) -> f64 {
        self.width_m
    }
}

struct Prism {
    length_m: f64,
    wall_m: f64,
    area_m2: f64,
}

impl OuterShape for Prism {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        let length = self.length_m - 2.0 * self.wall_m;
        if length <= 0.0 || self.area_m2 <= 0.0 {
            return Err("The prism has no air volume.".into());
        }
        Ok(self.area_m2 * length)
    }
    fn baffle_width_m(&self) -> f64 {
        self.area_m2.sqrt()
    }
}

pub struct TubeShape {
    pub inside_diameter_m: f64,
    pub outside_diameter_m: f64,
    pub length_m: f64,
    pub wall_from_user: bool,
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
    Ok(Box::new(RoundedBox {
        inner: box_from(fields)?,
        radius_m: required_f64(fields, "edge_radius_m")?,
    }))
}

fn parse_chamfered(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let _chamfer = required_f64(fields, "chamfer_m")?;
    Ok(Box::new(box_from(fields)?))
}

fn parse_sloped(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(SlopedBox {
        width_m: required_f64(fields, "width_m")?,
        height_m: required_f64(fields, "height_m")?,
        depth_front_m: required_f64(fields, "depth_front_m")?,
        depth_back_m: required_f64(fields, "depth_back_m")?,
        wall_m: required_f64(fields, "wall_m")?,
    }))
}

fn parse_prism(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let outline = fields
        .get("outline_m")
        .and_then(Value::as_array)
        .ok_or("missing outline_m")?;
    let mut area = 0.0;
    for index in 0..outline.len() {
        let next = (index + 1) % outline.len();
        let (x0, y0) = point2(&outline[index]);
        let (x1, y1) = point2(&outline[next]);
        area += x0 * y1 - x1 * y0;
    }
    Ok(Box::new(Prism {
        length_m: required_f64(fields, "length_m")?,
        wall_m: required_f64(fields, "wall_m")?,
        area_m2: area.abs() / 2.0,
    }))
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
    Ok(TubeShape {
        inside_diameter_m,
        outside_diameter_m,
        length_m,
        wall_from_user,
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
