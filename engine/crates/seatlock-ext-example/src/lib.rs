//! Test-only outer shape, radiator, and DSP block for CHK-EXT-1.

use num_complex::Complex64;
use seatlock_core::registry::{DspBlock, OuterShape, Radiator, RadiatorEval, Registry};
use serde_json::{Map, Value};
use std::f64::consts::PI;

pub fn register(registry: &mut Registry) {
    registry.register_outer("test_cylinder", parse_cylinder);
    registry.register_radiator("test_constant_source", parse_source);
    registry.register_dsp("test_half_gain", parse_half);
}

struct Cylinder {
    radius_m: f64,
    height_m: f64,
}

impl OuterShape for Cylinder {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(PI * self.radius_m * self.radius_m * self.height_m)
    }

    fn baffle_width_m(&self) -> f64 {
        self.radius_m * 2.0
    }
}

fn parse_cylinder(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    let radius_m = fields
        .get("radius_m")
        .and_then(Value::as_f64)
        .ok_or("missing radius_m")?;
    let height_m = fields
        .get("height_m")
        .and_then(Value::as_f64)
        .ok_or("missing height_m")?;
    Ok(Box::new(Cylinder { radius_m, height_m }))
}

struct ConstantSource {
    u_per_volt: f64,
}

impl Radiator for ConstantSource {
    fn evaluate(&self, freqs_hz: &[f64], _z_ab: &[Complex64]) -> RadiatorEval {
        let n = freqs_hz.len();
        RadiatorEval {
            volume_velocity: vec![Complex64::new(self.u_per_volt, 0.0); n],
            impedance: vec![Complex64::new(8.0, 0.0); n],
            excursion_m: vec![0.0; n],
            current_a: vec![Complex64::new(0.0, 0.0); n],
            re_ohm: 8.0,
            xmax_m: 1.0,
            pe_w: 1.0,
            has_excursion_limit: false,
            warnings: Vec::new(),
        }
    }
}

fn parse_source(fields: &Map<String, Value>) -> Result<Box<dyn Radiator>, String> {
    let u_per_volt = fields
        .get("u_per_volt")
        .and_then(Value::as_f64)
        .ok_or("missing u_per_volt")?;
    Ok(Box::new(ConstantSource { u_per_volt }))
}

struct HalfGain;

impl DspBlock for HalfGain {
    fn frequency_response(&self, freqs_hz: &[f64], _sample_rate_hz: f64) -> Vec<Complex64> {
        vec![Complex64::new(0.5, 0.0); freqs_hz.len()]
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            *slot = sample * 0.5;
        }
    }

    fn reset(&mut self) {}
}

fn parse_half(_fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    Ok(Box::new(HalfGain))
}
