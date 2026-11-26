//! One-dimensional acoustic network: ducts, chambers, and radiation loads.

use crate::passive_radiator::PassiveRadiatorMech;
use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

#[derive(Clone, Debug)]
pub enum AcousticNode {
    /// Rigid termination. Volume velocity at this end is zero.
    Rigid,
    /// Opening into the air. `flush` is an end in a large wall; otherwise the end is free.
    Radiation { area_m2: f64, flush: bool },
    /// Opening whose radiation load is given (for example from a BEM solve). Still reports
    /// one unit volume velocity out, like [`AcousticNode::Radiation`].
    FixedRadiation { z: Complex64 },
    /// Passive radiator (Small Map/Cap/Rap) from a chamber to the outside (README §5). Reports
    /// one opening. `radiation` is an optional added radiation load (BEM); when `None` the
    /// branch is Map/Cap/Rap alone, matching the quick network (far-field radiation is applied
    /// to the opening afterward).
    PassiveRadiator {
        mech: PassiveRadiatorMech,
        radiation: Option<Complex64>,
    },
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
        AcousticNode::FixedRadiation { z } => WaveguideResult {
            z_in: *z,
            openings: vec![Complex64::new(1.0, 0.0)],
            pressures: Vec::new(),
        },
        AcousticNode::PassiveRadiator { mech, radiation } => {
            let freq = omega / (2.0 * PI);
            let z = mech.acoustic_impedance(freq) + radiation.unwrap_or_default();
            WaveguideResult {
                z_in: z,
                openings: vec![Complex64::new(1.0, 0.0)],
                pressures: Vec::new(),
            }
        }
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

/// Analytic mouth radiation impedance (README 5.4): free end or flush with a wall, for `ka < 1`.
pub fn radiation_impedance(area_m2: f64, flush: bool, k: f64) -> Complex64 {
    let radius = (area_m2 / PI).sqrt();
    let ka = k * radius;
    let z0 = RHO0 * C0 / area_m2.max(1e-12);
    if flush {
        z0 * Complex64::new(0.5 * ka * ka, 0.849 * ka)
    } else {
        z0 * Complex64::new(0.25 * ka * ka, 0.613 * ka)
    }
}

/// Replace each [`AcousticNode::Radiation`] leaf, in depth-first order, with
/// [`AcousticNode::FixedRadiation`] from `loads`. Leaves past the end of `loads` keep the
/// analytic formula (resolved later at solve time). Used to join BEM mouth loads (self or
/// effective mutual) into the waveguide network.
pub fn with_fixed_radiation_loads(node: &AcousticNode, loads: &[Complex64]) -> AcousticNode {
    let mut index = 0;
    replace_radiation(node, loads, &mut index)
}

/// Effective radiation load for each mouth from a full impedance matrix and mouth volume
/// velocities: `Z_eff[i] = (Z U)_i / U_i`. Tiny `U_i` keeps the self term `Z[i][i]`.
pub fn effective_radiation_loads(z: &[Vec<Complex64>], openings: &[Complex64]) -> Vec<Complex64> {
    let mut loads = Vec::with_capacity(z.len());
    for (i, row) in z.iter().enumerate() {
        let mut pressure = Complex64::new(0.0, 0.0);
        for (j, &zij) in row.iter().enumerate() {
            let u = openings.get(j).copied().unwrap_or_default();
            pressure += zij * u;
        }
        let u_i = openings.get(i).copied().unwrap_or_default();
        if u_i.norm() < 1e-18 {
            loads.push(row.get(i).copied().unwrap_or_default());
        } else {
            loads.push(pressure / u_i);
        }
    }
    loads
}

/// Solve a waveguide tree whose radiation leaves are terminated by the full mouth impedance
/// matrix `z_rad` (DFS leaf order). Starts from the diagonal and iterates effective loads
/// until the opening velocities settle (or eight passes). One mouth, or a diagonal matrix,
/// matches [`with_fixed_radiation_loads`] in one step.
pub fn solve_with_radiation_matrix(
    node: &AcousticNode,
    freq_hz: f64,
    z_rad: &[Vec<Complex64>],
    lossy: bool,
    walls: &[Complex64],
) -> WaveguideResult {
    let n = z_rad.len();
    if n == 0 {
        return if lossy {
            solve_with_walls(node, freq_hz, true, walls)
        } else if walls.is_empty() {
            solve(node, freq_hz)
        } else {
            solve_with_walls(node, freq_hz, false, walls)
        };
    }
    let mut loads: Vec<Complex64> = (0..n)
        .map(|i| z_rad[i].get(i).copied().unwrap_or_default())
        .collect();
    let mut solved = {
        let rooted = with_fixed_radiation_loads(node, &loads);
        solve_with_walls(&rooted, freq_hz, lossy, walls)
    };
    for _ in 0..8 {
        if solved.openings.len() != n {
            break;
        }
        let next = effective_radiation_loads(z_rad, &solved.openings);
        let mut settled = true;
        for (old, new) in loads.iter().zip(next.iter()) {
            let scale = old.norm().max(new.norm()).max(1e-12);
            if (old - new).norm() / scale > 1e-6 {
                settled = false;
                break;
            }
        }
        loads = next;
        let rooted = with_fixed_radiation_loads(node, &loads);
        solved = solve_with_walls(&rooted, freq_hz, lossy, walls);
        if settled {
            break;
        }
    }
    solved
}

fn replace_radiation(node: &AcousticNode, loads: &[Complex64], index: &mut usize) -> AcousticNode {
    match node {
        AcousticNode::Radiation { area_m2, flush } => {
            let slot = *index;
            *index += 1;
            match loads.get(slot) {
                Some(z) => AcousticNode::FixedRadiation { z: *z },
                None => AcousticNode::Radiation {
                    area_m2: *area_m2,
                    flush: *flush,
                },
            }
        }
        AcousticNode::PassiveRadiator { mech, radiation } => {
            let slot = *index;
            *index += 1;
            match loads.get(slot) {
                Some(z) => AcousticNode::PassiveRadiator {
                    mech: *mech,
                    radiation: Some(*z),
                },
                None => AcousticNode::PassiveRadiator {
                    mech: *mech,
                    radiation: *radiation,
                },
            }
        }
        AcousticNode::FixedRadiation { z } => AcousticNode::FixedRadiation { z: *z },
        AcousticNode::Rigid => AcousticNode::Rigid,
        AcousticNode::Compliance { volume_m3 } => AcousticNode::Compliance {
            volume_m3: *volume_m3,
        },
        AcousticNode::Chamber { volume_m3, slot } => AcousticNode::Chamber {
            volume_m3: *volume_m3,
            slot: *slot,
        },
        AcousticNode::Duct {
            length_m,
            area_in_m2,
            area_out_m2,
            added_length_m,
            load,
        } => AcousticNode::Duct {
            length_m: *length_m,
            area_in_m2: *area_in_m2,
            area_out_m2: *area_out_m2,
            added_length_m: *added_length_m,
            load: Box::new(replace_radiation(load, loads, index)),
        },
        AcousticNode::Parallel(branches) => AcousticNode::Parallel(
            branches
                .iter()
                .map(|branch| replace_radiation(branch, loads, index))
                .collect(),
        ),
    }
}

/// One duct in a general (possibly looped) chamber network.
#[derive(Clone, Debug)]
pub struct MeshDuct {
    pub from: usize,
    /// `None` when the far end opens outside.
    pub to: Option<usize>,
    pub length_m: f64,
    pub area_in_m2: f64,
    pub area_out_m2: f64,
    pub added_length_m: f64,
    /// Optional moulded slices `(length, area)` from chamber to mouth.
    pub slices: Option<Vec<(f64, f64)>>,
    pub outside_slot: Option<usize>,
    pub mouth_area_m2: f64,
    /// When set on an outside branch, Small Map/Cap/Rap instead of a duct to radiation.
    pub passive: Option<PassiveRadiatorMech>,
}

/// Chambers plus ducts that may form loops (README 5.4 Level 3).
#[derive(Clone, Debug)]
pub struct WaveguideMesh {
    pub volumes_m3: Vec<f64>,
    pub edges: Vec<MeshDuct>,
    pub driver: usize,
}

/// Solve a chamber–duct mesh. Outside openings are listed by ascending `outside_slot`.
/// Optional `radiation_loads` replaces analytic mouth Z in that slot order (BEM join).
pub fn solve_mesh(
    mesh: &WaveguideMesh,
    freq_hz: f64,
    lossy: bool,
    walls: &[Complex64],
    radiation_loads: Option<&[Complex64]>,
) -> WaveguideResult {
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let k = omega / C0;
    let loss = if lossy {
        WALL_LOSS * freq_hz.max(0.0).sqrt()
    } else {
        0.0
    };
    let n = mesh.volumes_m3.len();
    let zero = Complex64::new(0.0, 0.0);
    let mut y = vec![vec![zero; n]; n];
    for (i, volume) in mesh.volumes_m3.iter().enumerate() {
        y[i][i] += Complex64::new(0.0, omega * volume / (RHO0 * C0 * C0));
        y[i][i] += walls.get(i).copied().unwrap_or_default();
    }
    // Outside ducts: (design slot, chamber, edge index). Sorted for opening_order pairing.
    let mut outs: Vec<(usize, usize, usize)> = Vec::new();
    for (edge_i, edge) in mesh.edges.iter().enumerate() {
        match edge.to {
            Some(to) => {
                let (y11, y12, y22) = duct_y_parameters(
                    edge.length_m + edge.added_length_m,
                    edge.area_in_m2,
                    edge.area_out_m2,
                    edge.slices.as_deref(),
                    k,
                    loss,
                );
                let a = edge.from;
                let b = to;
                y[a][a] += y11;
                y[b][b] += y22;
                y[a][b] += y12;
                y[b][a] += y12;
            }
            None => {
                let slot = edge.outside_slot.unwrap_or(0);
                outs.push((slot, edge.from, edge_i));
            }
        }
    }
    outs.sort_by_key(|(slot, _, _)| *slot);
    for (out_i, (_slot, chamber, edge_i)) in outs.iter().enumerate() {
        let edge = &mesh.edges[*edge_i];
        let z_in = if let Some(pr) = edge.passive {
            let z_rad = radiation_loads
                .and_then(|loads| loads.get(out_i).copied())
                .unwrap_or_default();
            pr.acoustic_impedance(freq_hz) + z_rad
        } else {
            let z_rad = match radiation_loads.and_then(|loads| loads.get(out_i)) {
                Some(z) => *z,
                None => radiation_impedance(edge.mouth_area_m2, false, k),
            };
            duct_input_impedance(
                edge.length_m + edge.added_length_m,
                edge.area_in_m2,
                edge.area_out_m2,
                edge.slices.as_deref(),
                z_rad,
                k,
                loss,
            )
        };
        y[*chamber][*chamber] += 1.0 / z_in;
    }
    let mut rhs = vec![zero; n];
    rhs[mesh.driver] = Complex64::new(1.0, 0.0);
    let pressure = solve_linear(y, rhs);
    // Same convention as the tree solver: openings[i] pairs with opening_order[i].
    let mut openings = Vec::with_capacity(outs.len());
    for (out_i, (_slot, chamber, edge_i)) in outs.iter().enumerate() {
        let edge = &mesh.edges[*edge_i];
        if let Some(pr) = edge.passive {
            let z_rad = radiation_loads
                .and_then(|loads| loads.get(out_i).copied())
                .unwrap_or_default();
            let z_in = pr.acoustic_impedance(freq_hz) + z_rad;
            openings.push(pressure[*chamber] / z_in);
            continue;
        }
        let z_rad = match radiation_loads.and_then(|loads| loads.get(out_i)) {
            Some(z) => *z,
            None => radiation_impedance(edge.mouth_area_m2, false, k),
        };
        let far = WaveguideResult {
            z_in: z_rad,
            openings: vec![Complex64::new(1.0, 0.0)],
            pressures: Vec::new(),
        };
        let length = (edge.length_m + edge.added_length_m).max(1e-6);
        let driven = if let Some(slices) = &edge.slices {
            let mut node = far;
            for (index, (len, area)) in slices.iter().enumerate().rev() {
                // Chamber-end flush correction sits on the first slice (README 5.4).
                let added = if index == 0 {
                    0.849 * (area / PI).sqrt()
                } else {
                    0.0
                };
                node = duct_chain(*len + added, *area, *area, node, k, loss);
            }
            node
        } else {
            duct_chain(length, edge.area_in_m2, edge.area_out_m2, far, k, loss)
        };
        // Pressure at the chamber / Z_in of the duct branch = U into the duct; openings
        // in `driven` are already relative to that U.
        let u_in = pressure[*chamber] / driven.z_in;
        openings.push(driven.openings.first().copied().unwrap_or(zero) * u_in);
    }
    let pressures = pressure
        .iter()
        .enumerate()
        .map(|(slot, p)| (slot, *p))
        .collect();
    WaveguideResult {
        z_in: pressure[mesh.driver],
        openings,
        pressures,
    }
}

/// ABCD of a tapered duct (same slicing as [`duct_chain`]), then Y-parameters.
fn duct_y_parameters(
    length_m: f64,
    area_in_m2: f64,
    area_out_m2: f64,
    slices: Option<&[(f64, f64)]>,
    k: f64,
    loss: f64,
) -> (Complex64, Complex64, Complex64) {
    let (a, b, _c, d) = duct_abcd(length_m, area_in_m2, area_out_m2, slices, k, loss);
    // Y11 = D/B, Y12 = Y21 = -1/B, Y22 = A/B (reciprocal two-port).
    let inv_b = 1.0 / b;
    (d * inv_b, -inv_b, a * inv_b)
}

fn duct_input_impedance(
    length_m: f64,
    area_in_m2: f64,
    area_out_m2: f64,
    slices: Option<&[(f64, f64)]>,
    z_load: Complex64,
    k: f64,
    loss: f64,
) -> Complex64 {
    let far = WaveguideResult {
        z_in: z_load,
        openings: Vec::new(),
        pressures: Vec::new(),
    };
    if let Some(slices) = slices {
        let mut node = far;
        for (index, (len, area)) in slices.iter().enumerate().rev() {
            let added = if index == 0 {
                0.849 * (area / PI).sqrt()
            } else {
                0.0
            };
            node = duct_chain(*len + added, *area, *area, node, k, loss);
        }
        node.z_in
    } else {
        duct_chain(length_m.max(1e-6), area_in_m2, area_out_m2, far, k, loss).z_in
    }
}

fn duct_abcd(
    length_m: f64,
    area_in_m2: f64,
    area_out_m2: f64,
    slices: Option<&[(f64, f64)]>,
    k: f64,
    loss: f64,
) -> (Complex64, Complex64, Complex64, Complex64) {
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::new(0.0, 0.0);
    let mut a = one;
    let mut b = zero;
    let mut c = zero;
    let mut d = one;
    let mul = |a: Complex64,
               b: Complex64,
               c: Complex64,
               d: Complex64,
               a2: Complex64,
               b2: Complex64,
               c2: Complex64,
               d2: Complex64| {
        (
            a * a2 + b * c2,
            a * b2 + b * d2,
            c * a2 + d * c2,
            c * b2 + d * d2,
        )
    };
    let slice_abcd = |len: f64, area: f64| {
        let z0 = RHO0 * C0 / area.max(1e-12);
        let alpha = loss / (area.max(1e-12) / PI).sqrt();
        let kl = Complex64::new(k, -alpha) * len.max(1e-9);
        let cos = kl.cos();
        let sin = kl.sin();
        let j = Complex64::new(0.0, 1.0);
        (cos, j * z0 * sin, j * sin / z0, cos)
    };
    if let Some(slices) = slices {
        // Chamber → mouth: ABCD from near to far. Flush correction on the chamber end.
        for (index, (len, area)) in slices.iter().enumerate() {
            let added = if index == 0 {
                0.849 * (area / PI).sqrt()
            } else {
                0.0
            };
            let (a2, b2, c2, d2) = slice_abcd(*len + added, *area);
            (a, b, c, d) = mul(a, b, c, d, a2, b2, c2, d2);
        }
    } else {
        let length = length_m.max(1e-6);
        let pieces = ((length / 0.010).ceil() as usize).max(1);
        let piece = length / pieces as f64;
        for index in 0..pieces {
            let start = index as f64 / pieces as f64;
            let end = (index as f64 + 1.0) / pieces as f64;
            let area = area_in_m2 + (area_out_m2 - area_in_m2) * (start + end) * 0.5;
            let (a2, b2, c2, d2) = slice_abcd(piece, area);
            (a, b, c, d) = mul(a, b, c, d, a2, b2, c2, d2);
        }
    }
    (a, b, c, d)
}

fn solve_linear(mut a: Vec<Vec<Complex64>>, mut b: Vec<Complex64>) -> Vec<Complex64> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].norm().partial_cmp(&a[j][col].norm()).unwrap())
            .unwrap();
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let pivot_row = a[col].clone();
            for (cell, value) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *cell -= factor * value;
            }
            let value = b[col];
            b[row] -= factor * value;
        }
    }
    let mut x = vec![Complex64::new(0.0, 0.0); n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for (coefficient, value) in a[row].iter().zip(&x).skip(row + 1) {
            sum -= coefficient * value;
        }
        x[row] = sum / a[row][row];
    }
    x
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
