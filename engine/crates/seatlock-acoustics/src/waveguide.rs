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
    /// A chamber that reports its pressure under `slot`, and whose walls take the admittance
    /// given for that slot to [`solve_with_walls`] (README 5.11).
    Chamber { volume_m3: f64, slot: usize },
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
    /// Pressure in each [`AcousticNode::Chamber`] below this node, by slot, per unit volume
    /// velocity into this node.
    pub pressures: Vec<(usize, Complex64)>,
}

pub fn solve(node: &AcousticNode, freq_hz: f64) -> WaveguideResult {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let k = omega / C0;
    solve_at(node, k, omega, 0.0, &[])
}

/// README section 5.4: a smooth round duct of radius `a` loses `alpha = WALL_LOSS * sqrt(f) / a`
/// nepers per meter to the air's friction and heat flow at its wall.
pub const WALL_LOSS: f64 = 2.99e-5;

/// [`solve`] with every duct's wall loss: `k` becomes `k - j*alpha`, `alpha` taken for the
/// round duct of each slice's area.
pub fn solve_lossy(node: &AcousticNode, freq_hz: f64) -> WaveguideResult {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let k = omega / C0;
    solve_at(node, k, omega, WALL_LOSS * freq_hz.max(0.0).sqrt(), &[])
}

/// [`solve`] or [`solve_lossy`], with each chamber's walls a shunt: `walls[slot]` is the
/// outward volume velocity of that chamber's walls per pascal inside (0 for rigid walls).
pub fn solve_with_walls(
    node: &AcousticNode,
    freq_hz: f64,
    lossy: bool,
    walls: &[Complex64],
) -> WaveguideResult {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let loss = if lossy {
        WALL_LOSS * freq_hz.max(0.0).sqrt()
    } else {
        0.0
    };
    solve_at(node, omega / C0, omega, loss, walls)
}

/// `loss` is `alpha * a`: the wall loss per meter times the duct's radius (0 for none).
fn solve_at(
    node: &AcousticNode,
    k: f64,
    omega: f64,
    loss: f64,
    walls: &[Complex64],
) -> WaveguideResult {
    match node {
        AcousticNode::Rigid => WaveguideResult {
            z_in: Complex64::new(1.0e12, 0.0),
            openings: Vec::new(),
            pressures: Vec::new(),
        },
        AcousticNode::Radiation { area_m2, flush } => WaveguideResult {
            z_in: radiation_impedance(*area_m2, *flush, k),
            openings: vec![Complex64::new(1.0, 0.0)],
            pressures: Vec::new(),
        },
        AcousticNode::Compliance { volume_m3 } => {
            let admit = Complex64::new(0.0, omega * volume_m3 / (RHO0 * C0 * C0));
            WaveguideResult {
                z_in: 1.0 / admit,
                openings: Vec::new(),
                pressures: Vec::new(),
            }
        }
        AcousticNode::Chamber { volume_m3, slot } => {
            let wall = walls.get(*slot).copied().unwrap_or_default();
            let admit = Complex64::new(0.0, omega * volume_m3 / (RHO0 * C0 * C0)) + wall;
            let z_in = 1.0 / admit;
            WaveguideResult {
                z_in,
                openings: Vec::new(),
                pressures: vec![(*slot, z_in)],
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
            if loss == 0.0
                && matches!(load.as_ref(), AcousticNode::Rigid)
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
                    pressures: Vec::new(),
                };
            }
            let far = solve_at(load, k, omega, loss, walls);
            duct_chain(length, *area_in_m2, *area_out_m2, far, k, loss)
        }
        AcousticNode::Parallel(branches) => {
            let solved: Vec<WaveguideResult> = branches
                .iter()
                .map(|branch| solve_at(branch, k, omega, loss, walls))
                .collect();
            let mut admit = Complex64::new(0.0, 0.0);
            for branch in &solved {
                admit += 1.0 / branch.z_in;
            }
            let z_in = 1.0 / admit;
            let mut openings = Vec::new();
            let mut pressures = Vec::new();
            for branch in &solved {
                let share = z_in / branch.z_in;
                for opening in &branch.openings {
                    openings.push(share * *opening);
                }
                for (slot, p) in &branch.pressures {
                    pressures.push((*slot, share * *p));
                }
            }
            WaveguideResult {
                z_in,
                openings,
                pressures,
            }
        }
    }
}

fn duct_chain(
    length_m: f64,
    area_in_m2: f64,
    area_out_m2: f64,
    far: WaveguideResult,
    k: f64,
    loss: f64,
) -> WaveguideResult {
    let pieces = ((length_m / 0.010).ceil() as usize).max(1);
    let piece = length_m / pieces as f64;
    let mut z_load = far.z_in;
    let mut carry = far.openings;
    let mut pressures = far.pressures;
    for index in (0..pieces).rev() {
        let start = index as f64 / pieces as f64;
        let end = (index as f64 + 1.0) / pieces as f64;
        let area_here = area_in_m2 + (area_out_m2 - area_in_m2) * (start + end) * 0.5;
        let z0 = RHO0 * C0 / area_here.max(1e-12);
        // k - j*alpha, with alpha for the round duct of this slice's area.
        let alpha = loss / (area_here.max(1e-12) / PI).sqrt();
        let kl = Complex64::new(k, -alpha) * piece;
        let cos = kl.cos();
        let sin = kl.sin();
        let j = Complex64::new(0.0, 1.0);
        let denom = cos + j * sin / z0 * z_load;
        let u_ratio = 1.0 / denom;
        let z_in = (z_load * cos + j * z0 * sin) / denom;
        carry = carry.into_iter().map(|opening| opening * u_ratio).collect();
        for (_, p) in pressures.iter_mut() {
            *p *= u_ratio;
        }
        z_load = z_in;
    }
    WaveguideResult {
        z_in: z_load,
        openings: carry,
        pressures,
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
