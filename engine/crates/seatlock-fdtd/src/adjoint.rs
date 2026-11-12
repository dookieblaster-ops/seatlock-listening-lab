//! How `Z_ab` and every `H_k` change when a face inside the cabinet is opened or closed a
//! little: the gradient an optimizer needs to shape the inside (README section 5.5, "Shaping
//! the inside").
//!
//! The scheme of [`crate::grid`], with every face's open fraction `a` in `[0, 1]` in the
//! velocity update, is linear and time-invariant. With `z = e^(j*omega*dt)` and `X(z)` the
//! sum of `x[n] * z^-n` over all steps, the pressures obey `M(z) P = kp * s * Q`, where `Q`
//! is the pulse, `s` marks the source cells, and
//!
//! ```text
//! M(z) = diag(z*(1 + g) - (1 - g) + kp*L*(dt/2)*(z + 1)/(z - 1)) + kp*kv*z/(z - 1) * D^T A D
//! ```
//!
//! (`D` takes the pressure difference across each face, `A = diag(a)`, `g` and `L` are the
//! folded boundary admittance and the spherical-wave term of the special cells.) `M` is
//! symmetric, so the adjoint of this scheme is the scheme itself, and the field that carries
//! a change at a face back to the driver is the forward field. With `Delta_f(z)` the sum of
//! the pressure difference across face `f` at the start of each step:
//!
//! ```text
//! dP_out/da_f = -(z + 1)/(2*n_s) * kv*z/((z - 1)*Q) * Delta_f^2
//! dZ_ab/da_f  = dP_out/da_f * n_s*dx^2*Q / U_in^2
//! ```
//!
//! so a sealed cabinet needs the forward run alone. An opening's flow is
//! `O_k = -dx^2*kv*z/(z - 1) * r_k^T A D P` (`r_k` holds the opening's faces with their
//! signs). Its adjoint field comes from one more run of the same scheme, driven at the
//! cells on both sides of the opening by `D^T A r_k` times the pulse:
//!
//! ```text
//! dO_k/da_f = -dx^2*kv*z/(z - 1) * Delta_f * (r_kf - kv*z/((z - 1)*Q) * Delta'_kf)
//! dH_k/da_f = dO_k/da_f / U_in + O_k * (n_s*dx^2/(RHO0*C0)) * dP_out/da_f / U_in^2
//! ```
//!
//! These are exact for runs that have rung out; a run cut short leaves a tail out of every
//! sum. Without losses a pocket of air behind a nearly closed face rings almost forever, so
//! the sums can be taken with a decay instead: every signal is weighted by `e^(-s*t)`, which
//! is the same as `z = e^((s + j*omega)*dt)` in everything above. That is the response of the
//! same cabinet with a uniform loss in its air (every resonance widened by `s/pi` Hz), and a
//! run of `ln(1/quiet)/s` seconds then holds all of it, whatever the shape.
//!
//! The fields are `f64` here (the everyday solver's are `f32`), so that finite differences
//! can check the result.

use crate::grid::Grid;
use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::collections::HashMap;
use std::f64::consts::PI;

/// The Fourier sums over the design faces take every fourth step. The pulse (`sigma = 8*dt`)
/// holds nothing above a quarter of the sampling rate (down by `e^-39` there), so nothing
/// folds back.
const DECIMATE: usize = 4;

/// A face: `(axis, slot)`, the face between cell `slot` and the cell one step lower on `axis`.
pub type Face = (u8, usize);

pub struct Problem<'a> {
    pub grid: &'a Grid,
    /// Open fractions that differ from the grid's own 0/1 mask: `(axis, slot, a)`. Only faces
    /// between two air cells take one.
    pub relaxed: &'a [(u8, usize, f64)],
    /// The faces whose sensitivity is wanted.
    pub faces: &'a [Face],
    pub frequencies_hz: &'a [f64],
    pub max_steps: usize,
    /// The run ends once the driver's pressure has stayed below this fraction of its peak, or
    /// once the decay weight has fallen below it.
    pub quiet: f64,
    /// `s` in the weight `e^(-s*t)` on every signal, per second: a uniform loss in the air.
    /// Zero for the lossless response.
    pub decay_per_s: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Sensitivity {
    /// `Z_ab` per frequency.
    pub z: Vec<Complex64>,
    /// `H_k` per opening and frequency.
    pub h: Vec<Vec<Complex64>>,
    /// `dZ_ab/da_f`, at `face * frequencies + frequency`.
    pub dz: Vec<Complex64>,
    /// `dH_k/da_f` per opening, laid out like `dz`.
    pub dh: Vec<Vec<Complex64>>,
    pub steps: usize,
    /// False when the run reached `max_steps` before it rang out: the sums then miss a tail.
    pub decayed: bool,
    /// 3D runs taken: one, plus one per opening when faces were asked for.
    pub runs: usize,
}

/// The grid's open fractions per axis, with the relaxed faces in place.
pub fn open_fractions(grid: &Grid, relaxed: &[(u8, usize, f64)]) -> [Vec<f64>; 3] {
    let [nx, ny, nz] = grid.n;
    let cells = nx * ny * nz;
    let stride = [ny * nz, nz, 1usize];
    let faces_len = cells + ny * nz;
    let mut open = [
        vec![0.0; faces_len],
        vec![0.0; faces_len],
        vec![0.0; faces_len],
    ];
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = grid.cell(i, j, k);
                if !grid.air[c] {
                    continue;
                }
                let coords = [i, j, k];
                for axis in 0..3 {
                    if coords[axis] > 0 && grid.air[c - stride[axis]] {
                        open[axis][c] = 1.0;
                    }
                }
            }
        }
    }
    for &(axis, slot, a) in relaxed {
        if let Some(value) = open[axis as usize].get_mut(slot) {
            if *value != 0.0 {
                *value = a;
            }
        }
    }
    open
}

enum Drive {
    Source,
    Opening(usize),
}

struct March {
    p: Vec<Complex64>,
    u: Vec<Complex64>,
    q: Vec<Complex64>,
    out: Vec<Vec<Complex64>>,
    /// Fourier sums of the pressure difference across each face, `face * frequencies + m`.
    delta: Vec<Complex64>,
    source_cells: f64,
    steps: usize,
    decayed: bool,
}

struct Special {
    cell: usize,
    y_sum: f64,
    source: bool,
    force: f64,
    inductive: f64,
    integral: f64,
    updated: f64,
}

fn march(problem: &Problem, drive: Drive, fixed_steps: Option<usize>) -> March {
    let grid = problem.grid;
    let [nx, ny, nz] = grid.n;
    let cells = nx * ny * nz;
    let dx = grid.dx;
    let dt = grid.time_step_s();
    let kv = dt / (RHO0 * dx);
    let kp = RHO0 * C0 * C0 * dt / dx;
    let rho_c = RHO0 * C0;
    let stride = [ny * nz, nz, 1usize];
    let faces_len = cells + ny * nz;
    let open = open_fractions(grid, problem.relaxed);
    let span: [(usize, usize); 3] = std::array::from_fn(|axis| {
        let first = open[axis].iter().position(|x| *x != 0.0).unwrap_or(cells);
        let last = open[axis]
            .iter()
            .rposition(|x| *x != 0.0)
            .map_or(first, |x| x + 1);
        (first, last.max(first))
    });

    let mut special_of: HashMap<usize, usize> = HashMap::new();
    let mut specials: Vec<Special> = Vec::new();
    let mut touch = |cell: usize, specials: &mut Vec<Special>| -> usize {
        *special_of.entry(cell).or_insert_with(|| {
            specials.push(Special {
                cell,
                y_sum: 0.0,
                source: false,
                force: 0.0,
                inductive: 0.0,
                integral: 0.0,
                updated: 0.0,
            });
            specials.len() - 1
        })
    };
    for &cell in &grid.source {
        if cell < cells && grid.air[cell] {
            let s = touch(cell, &mut specials);
            specials[s].y_sum += 1.0 / rho_c;
            specials[s].source = true;
        }
    }
    for &(cell, count, r) in &grid.absorbing {
        if cell < cells && grid.air[cell] && count > 0 {
            let s = touch(cell, &mut specials);
            specials[s].y_sum += count as f64 / rho_c;
            specials[s].inductive += count as f64 / (RHO0 * r.max(2.0 * dx));
        }
    }
    match drive {
        Drive::Source => {
            for special in specials.iter_mut().filter(|s| s.source) {
                special.force = 1.0;
            }
        }
        Drive::Opening(index) => {
            // D^T A r_k: each face of the opening pushes on the cell at its high side and
            // pulls on the one at its low side.
            for &(axis, slot, sign) in &grid.openings[index] {
                let a = open[axis as usize][slot];
                if a == 0.0 || slot < stride[axis as usize] || slot >= cells {
                    continue;
                }
                let high = touch(slot, &mut specials);
                specials[high].force += a * sign;
                let low = touch(slot - stride[axis as usize], &mut specials);
                specials[low].force -= a * sign;
            }
        }
    }
    let source_cells = specials.iter().filter(|s| s.source).count() as f64;
    let air_span = (
        grid.air.iter().position(|a| *a).unwrap_or(cells),
        grid.air.iter().rposition(|a| *a).map_or(0, |x| x + 1),
    );

    let freqs = problem.frequencies_hz;
    let nf = freqs.len();
    let faces: Vec<(usize, usize)> = problem
        .faces
        .iter()
        .map(|&(axis, slot)| (slot, stride[axis as usize]))
        .collect();
    let mut delta = vec![Complex64::new(0.0, 0.0); faces.len() * nf];
    let mut phasor = vec![Complex64::new(0.0, 0.0); nf];

    let mut pressure = vec![0.0_f64; cells];
    let mut v = [
        vec![0.0_f64; faces_len],
        vec![0.0_f64; faces_len],
        vec![0.0_f64; faces_len],
    ];
    let sigma = 8.0 * dt;
    let t0 = 4.0 * sigma;
    let mut samples_p: Vec<f64> = Vec::new();
    let mut samples_u: Vec<f64> = Vec::new();
    let mut samples_q: Vec<f64> = Vec::new();
    let mut samples_out: Vec<Vec<f64>> = vec![Vec::new(); grid.openings.len()];
    let mut peak = 0.0_f64;
    let mut quiet_run = 0usize;
    let mut decayed = false;
    let mut steps = 0usize;
    let quiet_needed = 300usize.max((0.002 / dt) as usize);
    let limit = fixed_steps.unwrap_or(problem.max_steps);
    let (sx, sy) = (stride[0], stride[1]);

    while steps < limit {
        let time = steps as f64 * dt;
        let pulse = (-((time - t0) / sigma).powi(2)).exp();
        let weight = (-problem.decay_per_s * time).exp();
        if !faces.is_empty() && steps.is_multiple_of(DECIMATE) {
            for (w, f) in phasor.iter_mut().zip(freqs) {
                let angle = -2.0 * PI * f * time;
                *w = Complex64::new(angle.cos(), angle.sin());
            }
            for (sums, &(slot, s)) in delta.chunks_exact_mut(nf).zip(&faces) {
                let d = (pressure[slot] - pressure[slot - s]) * weight;
                if d != 0.0 {
                    for (sum, w) in sums.iter_mut().zip(&phasor) {
                        *sum += w * d;
                    }
                }
            }
        }
        for axis in 0..3 {
            let s = stride[axis];
            let (first, last) = span[axis];
            let mask = &open[axis];
            let vel = &mut v[axis];
            for c in first..last {
                vel[c] -= kv * mask[c] * (pressure[c] - pressure[c - s]);
            }
        }
        let [vx, vy, vz] = &v;
        let net = |c: usize| -> f64 { vx[c + sx] - vx[c] + vy[c + sy] - vy[c] + vz[c + 1] - vz[c] };
        let mut p_sum = 0.0;
        let mut u_sum = 0.0;
        for special in specials.iter_mut() {
            let old = pressure[special.cell];
            let g = 0.5 * kp * special.y_sum;
            let updated = (old * (1.0 - g)
                - kp * net(special.cell)
                - kp * special.inductive * special.integral
                + kp * special.force * pulse)
                / (1.0 + g);
            special.integral += 0.5 * dt * (old + updated);
            special.updated = updated;
            if special.source {
                let mean = 0.5 * (updated + old);
                p_sum += mean;
                u_sum += (special.force * pulse - mean / rho_c) * dx * dx;
            }
        }
        for (c, p) in pressure
            .iter_mut()
            .enumerate()
            .take(air_span.1)
            .skip(air_span.0)
        {
            *p -= kp * net(c);
        }
        for special in &specials {
            pressure[special.cell] = special.updated;
        }
        for (index, opening) in grid.openings.iter().enumerate() {
            let mut flow = 0.0;
            for &(axis, slot, sign) in opening {
                flow += v[axis as usize][slot] * sign;
            }
            samples_out[index].push(flow * dx * dx * weight);
        }
        let aperture_p = if source_cells > 0.0 {
            p_sum / source_cells
        } else {
            0.0
        };
        samples_p.push(aperture_p * weight);
        samples_u.push(u_sum * weight);
        samples_q.push(pulse * weight);
        peak = peak.max(aperture_p.abs());
        steps += 1;
        if fixed_steps.is_none() && time > t0 + 4.0 * sigma && peak > 0.0 {
            if aperture_p.abs() < problem.quiet * peak {
                quiet_run += 1;
            } else {
                quiet_run = 0;
            }
            if quiet_run >= quiet_needed || weight < problem.quiet {
                decayed = true;
                break;
            }
        }
    }
    for sum in delta.iter_mut() {
        *sum *= DECIMATE as f64;
    }
    let spectrum = |samples: &[f64]| -> Vec<Complex64> {
        freqs
            .iter()
            .map(|f| {
                let (re, im) = crate::grid::dft(samples, *f, dt);
                Complex64::new(re, im)
            })
            .collect()
    };
    March {
        p: spectrum(&samples_p),
        u: spectrum(&samples_u),
        q: spectrum(&samples_q),
        out: samples_out.iter().map(|o| spectrum(o)).collect(),
        delta,
        source_cells,
        steps,
        decayed,
    }
}

#[allow(clippy::needless_range_loop)]
/// `Z_ab` and `H_k`, and their derivatives with respect to the open fraction of each of
/// `problem.faces`: one run, plus one per opening when faces are given.
pub fn sensitivity(problem: &Problem) -> Sensitivity {
    let grid = problem.grid;
    let forward = march(problem, Drive::Source, None);
    let nf = problem.frequencies_hz.len();
    let faces = problem.faces.len();
    let dx = grid.dx;
    let dt = grid.time_step_s();
    let kv = dt / (RHO0 * dx);
    let n_s = forward.source_cells;
    let z: Vec<Complex64> = forward
        .p
        .iter()
        .zip(&forward.u)
        .map(|(p, u)| p / u)
        .collect();
    let h: Vec<Vec<Complex64>> = forward
        .out
        .iter()
        .map(|o| o.iter().zip(&forward.u).map(|(o, u)| o / u).collect())
        .collect();
    let mut result = Sensitivity {
        z,
        h,
        steps: forward.steps,
        decayed: forward.decayed,
        runs: 1,
        ..Sensitivity::default()
    };
    if faces == 0 {
        return result;
    }
    let z_of = |f: f64| Complex64::from_polar((problem.decay_per_s * dt).exp(), 2.0 * PI * f * dt);
    // kv*z/(z - 1) per frequency: what turns a pressure difference into a face velocity.
    let shift: Vec<Complex64> = problem
        .frequencies_hz
        .iter()
        .map(|f| {
            let zz = z_of(*f);
            kv * zz / (zz - 1.0)
        })
        .collect();
    let mut dp = vec![Complex64::new(0.0, 0.0); faces * nf];
    result.dz = vec![Complex64::new(0.0, 0.0); faces * nf];
    for face in 0..faces {
        for m in 0..nf {
            let zz = z_of(problem.frequencies_hz[m]);
            let d = forward.delta[face * nf + m];
            let value = -(zz + 1.0) / (2.0 * n_s) * shift[m] / forward.q[m] * d * d;
            dp[face * nf + m] = value;
            result.dz[face * nf + m] =
                value * n_s * dx * dx * forward.q[m] / (forward.u[m] * forward.u[m]);
        }
    }
    for (index, opening) in grid.openings.iter().enumerate() {
        let adjoint = march(problem, Drive::Opening(index), Some(forward.steps));
        result.runs += 1;
        let own: HashMap<Face, f64> = opening
            .iter()
            .map(|&(axis, slot, sign)| ((axis, slot), sign))
            .collect();
        let mut dh = vec![Complex64::new(0.0, 0.0); faces * nf];
        for (face, key) in problem.faces.iter().enumerate() {
            let r = own.get(key).copied().unwrap_or(0.0);
            for m in 0..nf {
                let at = face * nf + m;
                let u = forward.u[m];
                let d_out = -dx
                    * dx
                    * shift[m]
                    * forward.delta[at]
                    * (r - shift[m] / forward.q[m] * adjoint.delta[at]);
                dh[at] = d_out / u
                    + forward.out[index][m] * (n_s * dx * dx / (RHO0 * C0)) * dp[at] / (u * u);
            }
        }
        result.dh.push(dh);
    }
    result
}
