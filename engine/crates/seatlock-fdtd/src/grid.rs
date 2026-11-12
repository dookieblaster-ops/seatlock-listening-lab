//! General masked grid: any air region, a resistive source patch, openings into exterior air
//! blocks, and the flow through each opening (README section 5.5).

use num_complex::Complex64;
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

    /// Slot of the face on the low side of cell `(i, j, k)` along `axis`. Face arrays use the
    /// cell's own index: slot `c` along an axis is the face between cell `c` and the cell one
    /// step lower on that axis.
    pub fn face_slot(&self, _axis: u8, i: usize, j: usize, k: usize) -> usize {
        self.cell(i, j, k)
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
    /// Stop early once the fitted free decay predicts the spectrum (module `extrapolate`).
    pub extrapolate: bool,
    /// For the early stop: at each frequency, the size of `Z_ab` that matters to the driver
    /// (its own mechanical impedance seen from the air, `Z_m / S_d^2`). Changes in `Z_ab`
    /// are measured against `|Z_ab|` plus this. Empty: against `|Z_ab|` alone.
    pub z_scale: Vec<f64>,
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
    /// True when the run stopped early and the rest of each signal was fitted.
    pub extrapolated: bool,
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
    let faces_len = cells + ny * nz;
    // Open faces as a 0/1 mask per axis, so the update runs as plain dense loops.
    let mut open: [Vec<f32>; 3] = [
        vec![0.0; faces_len],
        vec![0.0; faces_len],
        vec![0.0; faces_len],
    ];
    let mut special_of = vec![u32::MAX; cells];
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
    // The first and last cells along each axis whose low face can be open.
    let span: [(usize, usize); 3] = std::array::from_fn(|axis| {
        let first = open[axis].iter().position(|x| *x != 0.0).unwrap_or(cells);
        let last = open[axis]
            .iter()
            .rposition(|x| *x != 0.0)
            .map_or(first, |x| x + 1);
        (first, last.max(first))
    });

    // Special cells: source faces and absorbing faces.
    struct Special {
        cell: usize,
        y_sum: f64,
        source: bool,
        inductive: f64,
        integral: f64,
        updated: f64,
    }
    let mut specials: Vec<Special> = Vec::new();
    let mut touch = |cell: usize, specials: &mut Vec<Special>| -> usize {
        if special_of[cell] == u32::MAX {
            special_of[cell] = specials.len() as u32;
            specials.push(Special {
                cell,
                y_sum: 0.0,
                source: false,
                inductive: 0.0,
                integral: 0.0,
                updated: 0.0,
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
    let air_span = (
        grid.air.iter().position(|a| *a).unwrap_or(cells),
        grid.air.iter().rposition(|a| *a).map_or(0, |x| x + 1),
    );

    let mut pressure = vec![0.0_f32; cells];
    let mut v = [
        vec![0.0_f32; faces_len],
        vec![0.0_f32; faces_len],
        vec![0.0_f32; faces_len],
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
    // Early stop: fit the free decay over the last 40 ms every 10 ms of simulated time, from
    // 30 ms after the pulse, and stop when two fits in a row agree with the one before.
    let freqs = &options.frequencies_hz;
    let f_max = freqs.iter().cloned().fold(0.0, f64::max);
    let quiet_from = ((t0 + 4.0 * sigma) / dt).ceil() as usize + 1;
    let check_every = ((0.01 / dt) as usize).max(1);
    let fit_span = (0.12 / dt) as usize;
    let check_from = quiet_from + (0.03 / dt) as usize;
    let mut previous: Option<Spectra> = None;
    let mut agreed = 0usize;
    let mut stopped_early: Option<Spectra> = None;
    let mut running = crate::extrapolate::RunningSpectra::new(2 + grid.openings.len(), freqs, dt);
    let (sx, sy) = (stride[0], stride[1]);

    while steps < options.max_steps {
        let time = steps as f64 * dt;
        let pulse = (-((time - t0) / sigma).powi(2)).exp();
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
        let net = |c: usize| -> f32 { vx[c + sx] - vx[c] + vy[c + sy] - vy[c] + vz[c + 1] - vz[c] };
        // Special cells first, from the face velocities alone, then the dense update; the
        // special values then replace what the dense loop wrote.
        let mut p_sum = 0.0;
        let mut u_sum = 0.0;
        let mut source_faces = 0.0;
        for special in specials.iter_mut() {
            let old = pressure[special.cell] as f64;
            let outflow = net(special.cell) as f64;
            let v_source = if special.source { pulse } else { 0.0 };
            let g = 0.5 * kp * special.y_sum;
            let updated =
                (old * (1.0 - g) - kp * outflow - kp * special.inductive * special.integral
                    + kp * v_source)
                    / (1.0 + g);
            special.integral += 0.5 * dt * (old + updated);
            special.updated = updated;
            if special.source {
                let mean = 0.5 * (updated + old);
                p_sum += mean;
                u_sum += (v_source - mean / rho_c) * dx * dx;
                source_faces += 1.0;
            }
        }
        // Cells that are not air have only closed faces, so their pressure stays zero.
        for (c, p) in pressure
            .iter_mut()
            .enumerate()
            .take(air_span.1)
            .skip(air_span.0)
        {
            *p -= kp32 * net(c);
        }
        for special in &specials {
            pressure[special.cell] = special.updated as f32;
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
        if options.extrapolate {
            let mut now = vec![aperture_p, u_sum];
            now.extend(samples_out.iter().map(|o| *o.last().unwrap_or(&0.0)));
            running.push(&now);
        }
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
        if options.extrapolate && steps >= check_from && steps.is_multiple_of(check_every) {
            let window = crate::extrapolate::Window {
                start: steps.saturating_sub(fit_span).max(quiet_from),
                end: steps,
                dt,
                f_max_hz: f_max,
            };
            let mut channels: Vec<&[f64]> = vec![&samples_p, &samples_u];
            channels.extend(samples_out.iter().map(|o| o.as_slice()));
            let estimate = extrapolated(&channels, &running, &window, freqs);
            match (&estimate, &previous) {
                (Some(now), Some(before)) if agrees(now, before, freqs, &options.z_scale) => {
                    agreed += 1;
                }
                _ => agreed = 0,
            }
            previous = estimate;
            if agreed >= 2 {
                stopped_early = previous.take();
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
    if let Some(spectra) = stopped_early {
        let mut response = Response {
            frequencies_hz: freqs.clone(),
            h_real: vec![Vec::new(); grid.openings.len()],
            h_imag: vec![Vec::new(); grid.openings.len()],
            steps,
            decayed: true,
            extrapolated: true,
            canceled: false,
            ..Response::default()
        };
        for (bin, z) in spectra.z.iter().enumerate() {
            response.z_real.push(z.re);
            response.z_imag.push(z.im);
            for (index, h) in spectra.h.iter().enumerate() {
                response.h_real[index].push(h[bin].re);
                response.h_imag[index].push(h[bin].im);
            }
        }
        response.elapsed_ms = crate::elapsed_ms(started);
        return response;
    }
    let mut response = Response {
        frequencies_hz: Vec::new(),
        z_real: Vec::new(),
        z_imag: Vec::new(),
        h_real: vec![Vec::new(); grid.openings.len()],
        h_imag: vec![Vec::new(); grid.openings.len()],
        steps,
        decayed,
        extrapolated: false,
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

/// `Z_ab` and each opening's `H_k` at the requested frequencies.
struct Spectra {
    z: Vec<Complex64>,
    h: Vec<Vec<Complex64>>,
}

/// The spectra with each signal's fitted free decay added past the last step. Channel 0 is
/// the driver's pressure, 1 its flow, and the rest the openings' flows.
fn extrapolated(
    channels: &[&[f64]],
    running: &crate::extrapolate::RunningSpectra,
    window: &crate::extrapolate::Window,
    freqs: &[f64],
) -> Option<Spectra> {
    let fit = crate::extrapolate::Fit::new(channels, window)?;
    let solver = fit.shared()?;
    let full = |index: usize| {
        fit.extended_with(
            index,
            window.end,
            window.dt,
            freqs,
            &running.sums[index],
            &solver,
        )
    };
    let p_hat = full(0)?;
    let u_hat = full(1)?;
    let z = p_hat.iter().zip(&u_hat).map(|(a, b)| a / b).collect();
    let mut h = Vec::new();
    for index in 2..channels.len() {
        let o_hat = full(index)?;
        h.push(o_hat.iter().zip(&u_hat).map(|(a, b)| a / b).collect());
    }
    Some(Spectra { z, h })
}

/// Two estimates agree when `Z_ab` and every `H_k`, power-averaged over a sixth of an octave
/// around each frequency, differ by under 0.05 dB everywhere, each measured against what it
/// adds to (see `worst_change`). The heights of single sharp resonances are left out on purpose: without losses they
/// are set by the run's length, not the cabinet (README 5.5), and the labels say so.
fn agrees(a: &Spectra, b: &Spectra, freqs: &[f64], z_scale: &[f64]) -> bool {
    worst_change(a, b, freqs, z_scale).0 <= 0.05
}

/// Power average of `values` over a sixth of an octave around each frequency.
fn smoothed(values: &[Complex64], freqs: &[f64]) -> Vec<f64> {
    let ratio = 2f64.powf(1.0 / 12.0);
    freqs
        .iter()
        .map(|f| {
            let (mut sum, mut count) = (0.0_f64, 0.0_f64);
            for (v, g) in values.iter().zip(freqs) {
                if *g >= f / ratio && *g <= f * ratio {
                    sum += v.norm_sqr();
                    count += 1.0;
                }
            }
            (sum / count.max(1.0)).sqrt()
        })
        .collect()
}

/// The largest change in dB between two estimates' smoothed sizes, and where. Each size has
/// its floor added first: `z_scale` for `Z_ab` (none when empty), and 1 for each `H_k`, the
/// cone's own output, since a port's flow adds to it (`U_total = U_d * (1 - sum of H_k)`).
fn worst_change(a: &Spectra, b: &Spectra, freqs: &[f64], z_scale: &[f64]) -> (f64, usize) {
    let mut worst = (0.0, 0);
    let mut compare = |x: &[Complex64], y: &[Complex64], floor: &dyn Fn(usize) -> f64| {
        let (sx, sy) = (smoothed(x, freqs), smoothed(y, freqs));
        for (bin, (p, q)) in sx.iter().zip(&sy).enumerate() {
            let f = floor(bin);
            let d = (20.0 * ((p + f) / (q + f).max(1e-300)).log10()).abs();
            if d > worst.0 {
                worst = (d, bin);
            }
        }
    };
    compare(&a.z, &b.z, &|bin| z_scale.get(bin).copied().unwrap_or(0.0));
    for (x, y) in a.h.iter().zip(&b.h) {
        compare(x, y, &|_| 1.0);
    }
    worst
}

/// Discrete-time Fourier transform of `samples` at one frequency, `sum x[n] e^(-j w n dt)`.
pub(crate) fn dft(samples: &[f64], freq: f64, dt: f64) -> (f64, f64) {
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
