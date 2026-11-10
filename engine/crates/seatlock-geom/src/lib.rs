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
}

fn parse_box(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(BoxShape {
        width_m: required_f64(fields, "width_m")?,
        height_m: required_f64(fields, "height_m")?,
        depth_m: required_f64(fields, "depth_m")?,
        wall_m: required_f64(fields, "wall_m")?,
    }))
}

fn required_f64(fields: &Map<String, Value>, key: &str) -> Result<f64, String> {
    fields
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing {key}"))
}
