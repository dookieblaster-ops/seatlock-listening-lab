//! General masked grid: any air region, a resistive source patch, openings into exterior air
//! blocks, and the flow through each opening (README section 5.5).

use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

/// A grid of cubic cells. Cell `(i, j, k)` has index `(i * ny + j) * nz + k`, and its center is at
/// `origin_m + (index + 0.5) * dx` along each axis.
#[derive(Clone, Debug)]
pub struct Grid {
    pub n: [usize; 3],
    pub dx: f64,
    pub origin_m: [f64; 3],
    /// True where the cell is air (inside the cabinet or in an exterior block).
    pub air: Vec<bool>,
    /// Cells whose wall face is the driver aperture, each counted as one face of `dx^2`.
    pub source: Vec<usize>,
    /// Absorbing faces of exterior cells: `(cell, face count, distance in m from the opening's
    /// mouth to the cell center)`. See [`run`] for the boundary condition.
    pub absorbing: Vec<(usize, u8, f64)>,
    /// One entry per opening: the faces across it, `(axis, face slot, sign)`, where the sign
    /// makes positive flow leave the cabinet.
    pub openings: Vec<Vec<(u8, usize, f64)>>,
    /// Air volume of the cabinet as the design gives it, for the self-check.
    pub true_volume_m3: f64,
}

impl Grid {
    pub fn cell(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.n[1] + j) * self.n[2] + k
    }

    /// Slot of the face on the low side of cell `(i, j, k)` along `axis`.
    pub fn face_slot(&self, axis: u8, i: usize, j: usize, k: usize) -> usize {
        let [_, ny, nz] = self.n;
        match axis {
            0 => (i * ny + j) * nz + k,
            1 => (i * (ny + 1) + j) * nz + k,
            _ => (i * ny + j) * (nz + 1) + k,
        }
    }

    pub fn time_step_s(&self) -> f64 {
        0.95 * self.dx / (C0 * 3.0_f64.sqrt())
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub max_steps: usize,
    /// Frequencies at which `Z_ab` and `H_k` are returned.
    pub frequencies_hz: Vec<f64>,
    /// Steps between progress calls.
    pub progress_every: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Response {
    pub frequencies_hz: Vec<f64>,
    pub z_real: Vec<f64>,
    pub z_imag: Vec<f64>,
    /// `H_k` per opening.
    pub h_real: Vec<Vec<f64>>,
    pub h_imag: Vec<Vec<f64>>,
    pub steps: usize,
    pub decayed: bool,
    pub canceled: bool,
    pub elapsed_ms: f64,
}

/// Runs the grid. `progress(steps, max_steps)` is called every `progress_every` steps and stops
/// the run when it returns false.
///
/// Absorbing faces use the admittance of an outgoing spherical wave centered on the opening's
/// mouth, `Y = (1 + 1/(j*k*r)) / (RHO0*C0)`, so the boundary velocity is
/// `p/(RHO0*C0) + (1/(RHO0*r)) * integral of p dt`. A plane-wave face (`1/(RHO0*C0)` alone) is
/// resistive at every frequency, while the near field of a small opening is mostly reactive at
/// low frequency, so a block a few widths across added a false resistance to the radiation load
/// and the result depended on the block size (STATUS, 2026-11-10).
pub fn run(
    grid: &Grid,
    options: &Options,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Response {
    let started = crate::clock_start();
    let [nx, ny, nz] = grid.n;
    let cells = nx * ny * nz;
    let dx = grid.dx;
    let dt = grid.time_step_s();
    let kv = (dt / (RHO0 * dx)) as f32;
    let kp = RHO0 * C0 * C0 * dt / dx;
    let kp32 = kp as f32;
    let rho_c = RHO0 * C0;

    let stride = [ny * nz, nz, 1usize];
    // Open faces, as (slot, cell ahead); the cell behind is `ahead - stride`.
    let mut faces: [Vec<(u32, u32)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut plain: Vec<(u32, u32, u32)> = Vec::new();
    let mut special_of = vec![u32::MAX; cells];
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = grid.cell(i, j, k);
                if !grid.air[c] {
                    continue;
                }
                let coords = [i, j, k];
                for axis in 0..3u8 {
                    if coords[axis as usize] > 0 && grid.air[c - stride[axis as usize]] {
                        faces[axis as usize].push((grid.face_slot(axis, i, j, k) as u32, c as u32));
                    }
                }
                plain.push((c as u32, i as u32, (i * ny + j) as u32));
            }
        }
    }

    // Special cells: source faces and absorbing faces.
    struct Special {
        cell: usize,
        i: usize,
        ij: usize,
        y_sum: f64,
        source: bool,
        inductive: f64,
        integral: f64,
    }
    let mut specials: Vec<Special> = Vec::new();
    let mut touch = |cell: usize, specials: &mut Vec<Special>| -> usize {
        if special_of[cell] == u32::MAX {
            let i = cell / (ny * nz);
            let j = (cell / nz) % ny;
            special_of[cell] = specials.len() as u32;
            specials.push(Special {
                cell,
                i,
                ij: i * ny + j,
                y_sum: 0.0,
                source: false,
                inductive: 0.0,
                integral: 0.0,
            });
        }
        special_of[cell] as usize
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
            // Keep the inductive term stable: never closer than two cells.
            specials[s].inductive += count as f64 / (RHO0 * r.max(2.0 * dx));
        }
    }
    plain.retain(|(c, _, _)| special_of[*c as usize] == u32::MAX);

    let mut pressure = vec![0.0_f32; cells];
    let mut v = [
        vec![0.0_f32; (nx + 1) * ny * nz],
        vec![0.0_f32; nx * (ny + 1) * nz],
        vec![0.0_f32; nx * ny * (nz + 1)],
    ];
    let sigma = 8.0 * dt;
    let t0 = 4.0 * sigma;
    let mut samples_p: Vec<f64> = Vec::new();
    let mut samples_u: Vec<f64> = Vec::new();
    let mut samples_out: Vec<Vec<f64>> = vec![Vec::new(); grid.openings.len()];
    let mut peak = 0.0_f64;
    let mut quiet_run = 0usize;
    let mut decayed = false;
    let mut canceled = false;
    let mut steps = 0usize;
    let quiet_needed = 300usize.max((0.002 / dt) as usize);

    while steps < options.max_steps {
        let time = steps as f64 * dt;
        let pulse = (-((time - t0) / sigma).powi(2)).exp();
        for axis in 0..3 {
            let s = stride[axis] as u32;
            let vel = &mut v[axis];
            for &(slot, ahead) in &faces[axis] {
                let diff = pressure[ahead as usize] - pressure[(ahead - s) as usize];
                vel[slot as usize] -= kv * diff;
            }
        }
        let [vx, vy, vz] = &v;
        let net = |c: usize, i: usize, ij: usize| -> f32 {
            let sx = c;
            let sy = c + i * nz;
            let sz = c + ij;
            vx[sx + ny * nz] - vx[sx] + vy[sy + nz] - vy[sy] + vz[sz + 1] - vz[sz]
        };
        // Plain cells update in place: a cell's new pressure depends only on face velocities.
        let mut p_sum = 0.0;
        let mut u_sum = 0.0;
        let mut source_faces = 0.0;
        let mut updates: Vec<(usize, f64)> = Vec::with_capacity(specials.len());
        for special in specials.iter_mut() {
            let old = pressure[special.cell] as f64;
            let outflow = net(special.cell, special.i, special.ij) as f64;
            let v_source = if special.source { pulse } else { 0.0 };
            let g = 0.5 * kp * special.y_sum;
            let updated =
                (old * (1.0 - g) - kp * outflow - kp * special.inductive * special.integral
                    + kp * v_source)
                    / (1.0 + g);
            special.integral += 0.5 * dt * (old + updated);
            if special.source {
                let mean = 0.5 * (updated + old);
                p_sum += mean;
                u_sum += (v_source - mean / rho_c) * dx * dx;
                source_faces += 1.0;
            }
            updates.push((special.cell, updated));
        }
        for &(c, i, ij) in &plain {
            let c = c as usize;
            pressure[c] -= kp32 * net(c, i as usize, ij as usize);
        }
        for (cell, value) in updates {
            pressure[cell] = value as f32;
        }
        for (index, opening) in grid.openings.iter().enumerate() {
            let mut flow = 0.0;
            for &(axis, slot, sign) in opening {
                flow += v[axis as usize][slot] as f64 * sign;
            }
            samples_out[index].push(flow * dx * dx);
        }
        let aperture_p = if source_faces > 0.0 {
            p_sum / source_faces
        } else {
            0.0
        };
        samples_p.push(aperture_p);
        samples_u.push(u_sum);
        peak = peak.max(aperture_p.abs());
        steps += 1;
        if time > t0 + 4.0 * sigma && peak > 0.0 {
            if aperture_p.abs() < 1e-4 * peak {
                quiet_run += 1;
            } else {
                quiet_run = 0;
            }
            if quiet_run >= quiet_needed {
                decayed = true;
                break;
            }
        }
        if options.progress_every > 0
            && steps.is_multiple_of(options.progress_every)
            && !progress(steps, options.max_steps)
        {
            canceled = true;
            break;
        }
    }

    let spectrum = |samples: &[f64], freq: f64| -> (f64, f64) { dft(samples, freq, dt) };
    let mut response = Response {
        frequencies_hz: Vec::new(),
        z_real: Vec::new(),
        z_imag: Vec::new(),
        h_real: vec![Vec::new(); grid.openings.len()],
        h_imag: vec![Vec::new(); grid.openings.len()],
        steps,
        decayed,
        canceled,
        elapsed_ms: 0.0,
    };
    for &freq in &options.frequencies_hz {
        let p = spectrum(&samples_p, freq);
        let u = spectrum(&samples_u, freq);
        let z = crate::div(p, u);
        response.frequencies_hz.push(freq);
        response.z_real.push(z.0);
        response.z_imag.push(z.1);
        for (index, out) in samples_out.iter().enumerate() {
            let h = crate::div(spectrum(out, freq), u);
            response.h_real[index].push(h.0);
            response.h_imag[index].push(h.1);
        }
    }
    response.elapsed_ms = crate::elapsed_ms(started);
    response
}

/// Discrete-time Fourier transform of `samples` at one frequency, `sum x[n] e^(-j w n dt)`.
fn dft(samples: &[f64], freq: f64, dt: f64) -> (f64, f64) {
    let step = -2.0 * PI * freq * dt;
    let (step_re, step_im) = (step.cos(), step.sin());
    let (mut w_re, mut w_im) = (1.0_f64, 0.0_f64);
    let (mut re, mut im) = (0.0, 0.0);
    for (index, &x) in samples.iter().enumerate() {
        re += x * w_re;
        im += x * w_im;
        let next = w_re * step_re - w_im * step_im;
        w_im = w_re * step_im + w_im * step_re;
        w_re = next;
        if index % 1024 == 1023 {
            // Renormalize the rotating phasor against drift.
            let norm = (w_re * w_re + w_im * w_im).sqrt();
            w_re /= norm;
            w_im /= norm;
        }
    }
    (re, im)
}

/// Logarithmic frequencies from `low` to `high`, `per_octave` points per octave.
pub fn log_frequencies(low: f64, high: f64, per_octave: f64) -> Vec<f64> {
    let count = ((high / low).log2() * per_octave).ceil() as usize;
    (0..=count)
        .map(|index| low * 2f64.powf(index as f64 / per_octave))
        .filter(|freq| *freq <= high * 1.000001)
        .collect()
}
