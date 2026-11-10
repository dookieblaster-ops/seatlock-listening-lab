//! Vented `lumped` box: a chamber compliance in parallel with one or more ports.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct PortSpec {
    pub area_m2: f64,
    pub length_eff_m: f64,
}

/// End correction for one end of a port. A flush wall uses 0.849 radii; a free end uses 0.613.
pub fn end_correction_m(radius_m: f64, flush_with_wall: bool) -> f64 {
    radius_m * if flush_with_wall { 0.849 } else { 0.613 }
}

pub fn effective_port_length_m(physical_m: f64, radius_m: f64) -> f64 {
    // Inside end flush with the chamber wall, outside end free.
    physical_m + end_correction_m(radius_m, true) + end_correction_m(radius_m, false)
}

#[derive(Clone, Copy, Debug)]
pub struct VentedSample {
    pub z_ab: Complex64,
    /// Sum of `H_k` over the ports. Approaches 1 as frequency goes to zero.
    pub h_sum: Complex64,
    /// `H_k` of each port, in the same order as the ports that were passed in.
    pub h_each: [Complex64; 4],
    pub port_count: usize,
}

/// Checks use no leak and no port resistance. `leak` turns on the default `q_leak` of 7.
pub fn vented_sample(volume_m3: f64, ports: &[PortSpec], freq_hz: f64, leak: bool) -> VentedSample {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let compliance = volume_m3 / (RHO0 * C0 * C0);
    let mut admit = Complex64::new(0.0, omega * compliance);
    let mut z_port = [Complex64::new(0.0, 0.0); 4];
    let count = ports.len().min(4);
    for (index, port) in ports.iter().take(count).enumerate() {
        let mass = RHO0 * port.length_eff_m / port.area_m2.max(1e-12);
        let impedance = Complex64::new(0.0, omega * mass);
        z_port[index] = impedance;
        admit += 1.0 / impedance;
    }
    if leak && count > 0 {
        let area: f64 = ports.iter().take(count).map(|port| port.area_m2).sum();
        let length: f64 = ports
            .iter()
            .take(count)
            .map(|port| port.length_eff_m)
            .sum::<f64>()
            / count as f64;
        let tuning = (C0 / (2.0 * PI)) * (area / (volume_m3 * length)).sqrt();
        let resistance = 7.0 / (2.0 * PI * tuning * compliance);
        admit += 1.0 / Complex64::new(resistance, 0.0);
    }
    let z_ab = 1.0 / admit;
    let mut h_each = [Complex64::new(0.0, 0.0); 4];
    let mut h_sum = Complex64::new(0.0, 0.0);
    for index in 0..count {
        h_each[index] = z_ab / z_port[index];
        h_sum += h_each[index];
    }
    VentedSample {
        z_ab,
        h_sum,
        h_each,
        port_count: count,
    }
}

pub fn tuning_hz(volume_m3: f64, ports: &[PortSpec]) -> f64 {
    let compliance = volume_m3.max(1e-9) / (RHO0 * C0 * C0);
    let mut inverse_mass = 0.0;
    for port in ports {
        let mass = RHO0 * port.length_eff_m.max(1e-6) / port.area_m2.max(1e-12);
        inverse_mass += 1.0 / mass;
    }
    if inverse_mass <= 0.0 {
        return 0.0;
    }
    1.0 / (2.0 * PI * (compliance / inverse_mass).sqrt())
}
