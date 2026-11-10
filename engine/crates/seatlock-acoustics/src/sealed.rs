use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

pub fn z_ab_sealed(volume_m3: f64, freq_hz: f64) -> Complex64 {
    let omega = 2.0 * PI * freq_hz;
    let compliance = volume_m3 / (RHO0 * C0 * C0);
    Complex64::new(0.0, -1.0 / (omega * compliance))
}
