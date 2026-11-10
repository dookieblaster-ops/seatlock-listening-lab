//! One-dimensional acoustic network: ducts, chambers, and radiation loads.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

#[derive(Clone, Debug)]
pub enum AcousticNode {
    /// Rigid termination. Volume velocity at this end is zero.
    Rigid,
    /// Opening into the air. `flush` is an end in a large wall; otherwise the end is free.
    Radiation { area_m2: f64, flush: bool },
    /// A chamber small enough to be a shunt compliance.
    Compliance { volume_m3: f64 },
    /// Uniform or tapered duct. A taper is sliced into pieces no longer than 10 mm.
    Duct {
        length_m: f64,
        area_in_m2: f64,
        area_out_m2: f64,
        /// Extra length for an end that opens through a wall into a chamber.
        added_length_m: f64,
        load: Box<AcousticNode>,
    },
    /// Branches that share one pressure.
    Parallel(Vec<AcousticNode>),
}

#[derive(Clone, Debug)]
pub struct WaveguideResult {
    pub z_in: Complex64,
    /// Volume velocity leaving each radiation opening, per unit volume velocity into this node.
    pub openings: Vec<Complex64>,
}

pub fn solve(node: &AcousticNode, freq_hz: f64) -> WaveguideResult {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let k = omega / C0;
    solve_at(node, k, omega)
}

fn solve_at(node: &AcousticNode, k: f64, omega: f64) -> WaveguideResult {
    match node {
        AcousticNode::Rigid => WaveguideResult {
            z_in: Complex64::new(1.0e12, 0.0),
            openings: Vec::new(),
        },
        AcousticNode::Radiation { area_m2, flush } => WaveguideResult {
            z_in: radiation_impedance(*area_m2, *flush, k),
            openings: vec![Complex64::new(1.0, 0.0)],
        },
        AcousticNode::Compliance { volume_m3 } => {
            let admit = Complex64::new(0.0, omega * volume_m3 / (RHO0 * C0 * C0));
            WaveguideResult {
                z_in: 1.0 / admit,
                openings: Vec::new(),
            }
        }
        AcousticNode::Duct {
            length_m,
            area_in_m2,
            area_out_m2,
            added_length_m,
            load,
        } => {
            let length = (*length_m + *added_length_m).max(1e-6);
            if matches!(load.as_ref(), AcousticNode::Rigid)
                && (area_in_m2 - area_out_m2).abs() < 1e-9
            {
                let z0 = RHO0 * C0 / area_in_m2.max(1e-12);
                let kl = k * length;
                let sin = kl.sin();
                let z_in = if sin.abs() < 1e-12 {
                    Complex64::new(1.0e12, 0.0)
                } else {
                    Complex64::new(0.0, -z0 * kl.cos() / sin)
                };
                return WaveguideResult {
                    z_in,
                    openings: Vec::new(),
                };
            }
            let far = solve_at(load, k, omega);
            duct_chain(length, *area_in_m2, *area_out_m2, far, k)
        }
        AcousticNode::Parallel(branches) => {
            let solved: Vec<WaveguideResult> = branches
                .iter()
                .map(|branch| solve_at(branch, k, omega))
                .collect();
            let mut admit = Complex64::new(0.0, 0.0);
            for branch in &solved {
                admit += 1.0 / branch.z_in;
            }
            let z_in = 1.0 / admit;
            let mut openings = Vec::new();
            for branch in &solved {
                let share = z_in / branch.z_in;
                for opening in &branch.openings {
                    openings.push(share * *opening);
                }
            }
            WaveguideResult { z_in, openings }
        }
    }
}

fn duct_chain(
    length_m: f64,
    area_in_m2: f64,
    area_out_m2: f64,
    far: WaveguideResult,
    k: f64,
) -> WaveguideResult {
    let pieces = ((length_m / 0.010).ceil() as usize).max(1);
    let piece = length_m / pieces as f64;
    let mut z_load = far.z_in;
    let mut carry = far.openings;
    for index in (0..pieces).rev() {
        let start = index as f64 / pieces as f64;
        let end = (index as f64 + 1.0) / pieces as f64;
        let area_here = area_in_m2 + (area_out_m2 - area_in_m2) * (start + end) * 0.5;
        let z0 = RHO0 * C0 / area_here.max(1e-12);
        let kl = k * piece;
        let cos = kl.cos();
        let sin = kl.sin();
        let denom = Complex64::new(cos, 0.0) + Complex64::new(0.0, sin / z0) * z_load;
        let u_ratio = 1.0 / denom;
        let z_in = (z_load * cos + Complex64::new(0.0, z0 * sin)) / denom;
        carry = carry.into_iter().map(|opening| opening * u_ratio).collect();
        z_load = z_in;
    }
    WaveguideResult {
        z_in: z_load,
        openings: carry,
    }
}

fn radiation_impedance(area_m2: f64, flush: bool, k: f64) -> Complex64 {
    let radius = (area_m2 / PI).sqrt();
    let ka = k * radius;
    let z0 = RHO0 * C0 / area_m2.max(1e-12);
    if flush {
        z0 * Complex64::new(0.5 * ka * ka, 0.849 * ka)
    } else {
        z0 * Complex64::new(0.25 * ka * ka, 0.613 * ka)
    }
}

/// A straight lossless duct closed at the far end. Used by CHK-WG-1.
pub fn closed_duct_impedance(length_m: f64, area_m2: f64, freq_hz: f64) -> Complex64 {
    solve(
        &AcousticNode::Duct {
            length_m,
            area_in_m2: area_m2,
            area_out_m2: area_m2,
            added_length_m: 0.0,
            load: Box::new(AcousticNode::Rigid),
        },
        freq_hz,
    )
    .z_in
}

/// Vented box: chamber compliance in parallel with one port. Inside end flush, outside end free.
pub fn ported_box(
    volume_m3: f64,
    radius_m: f64,
    physical_length_m: f64,
    freq_hz: f64,
) -> WaveguideResult {
    let area = PI * radius_m * radius_m;
    solve(
        &AcousticNode::Parallel(vec![
            AcousticNode::Compliance { volume_m3 },
            AcousticNode::Duct {
                length_m: physical_length_m,
                area_in_m2: area,
                area_out_m2: area,
                added_length_m: 0.849 * radius_m,
                load: Box::new(AcousticNode::Radiation {
                    area_m2: area,
                    flush: false,
                }),
            },
        ]),
        freq_hz,
    )
}

/// Chamber plus one neck. The inside end is flush with the chamber wall.
pub fn helmholtz(
    volume_m3: f64,
    radius_m: f64,
    physical_length_m: f64,
    freq_hz: f64,
) -> WaveguideResult {
    let area = PI * radius_m * radius_m;
    solve(
        &AcousticNode::Parallel(vec![
            AcousticNode::Compliance { volume_m3 },
            AcousticNode::Duct {
                length_m: physical_length_m,
                area_in_m2: area,
                area_out_m2: area,
                added_length_m: 0.849 * radius_m,
                load: Box::new(AcousticNode::Radiation {
                    area_m2: area,
                    flush: false,
                }),
            },
        ]),
        freq_hz,
    )
}
