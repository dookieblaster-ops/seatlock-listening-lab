use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

pub fn baffle_gain(space: &str, freq_hz: f64, baffle_width_m: f64) -> Complex64 {
    match space {
        "full" => Complex64::new(1.0, 0.0),
        "half" => Complex64::new(2.0, 0.0),
        "step" => {
            let f3 = 115.0 / baffle_width_m.max(1e-9);
            let numerator = Complex64::new(1.0, freq_hz / (f3 / 2.0_f64.sqrt()));
            let denominator = Complex64::new(1.0, freq_hz / (f3 * 2.0_f64.sqrt()));
            numerator / denominator
        }
        _ => Complex64::new(2.0, 0.0),
    }
}

pub fn pressure_from_volume_velocity(
    volume_velocity: Complex64,
    freq_hz: f64,
    distance_m: f64,
    space: &str,
    baffle_width_m: f64,
) -> Complex64 {
    let omega = 2.0 * PI * freq_hz;
    let wavenumber = omega / C0;
    let travel = Complex64::from_polar(1.0, -wavenumber * distance_m);
    let gain = baffle_gain(space, freq_hz, baffle_width_m);
    Complex64::i() * omega * RHO0 * volume_velocity * gain * travel / (4.0 * PI * distance_m)
}
