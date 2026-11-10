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
