//! Harmonic inversion of the 3D solver's recorded signals, so a run can stop long before its
//! echoes have died away.
//!
//! Once the source pulse is over, every recorded signal (the pressure at the driver, the flow
//! the driver pushes, the flow out of each opening) is a free decay of the same linear system:
//! a sum of damped sinusoids `sum c_i z_i^n` with poles `z_i` common to all of them. The poles
//! are found by the matrix pencil method on a recent stretch of the signals, low-passed and
//! decimated to the band of interest; each signal's amplitudes are then fitted at the full
//! rate, and the rest of its Fourier sum, from the last recorded step to infinity, is added in
//! closed form:
//!
//! ```text
//! sum over n >= N of c_i z_i^(n - n0) e^(-j w n dt) = c_i z_i^(N - n0) e^(-j w N dt) / (1 - z_i e^(-j w dt))
//! ```
//!
//! This is the approach of harmonic inversion (Mandelshtam and Taylor's filter
//! diagonalization; the matrix pencil of Hua and Sarkar is the same idea with an SVD). Signal
//! content above the band of interest is not modeled; it decays or is left as the residual of
//! the fit, which the convergence check in `grid::run` keeps small at the frequencies asked
//! for.

use nalgebra::{DMatrix, DVector};
use num_complex::Complex64;
use std::f64::consts::PI;

/// What a fit needs: where it starts and ends, the time step, and the band of interest.
pub struct Window {
    pub start: usize,
    pub end: usize,
    pub dt: f64,
    pub f_max_hz: f64,
}

/// Windowed-sinc low-pass taps, cutoff in cycles per sample.
fn lowpass(cutoff: f64, half: usize) -> Vec<f64> {
    let len = 2 * half + 1;
    let mut taps = Vec::with_capacity(len);
    for i in 0..len {
        let m = i as f64 - half as f64;
        let sinc = if m == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * PI * cutoff * m).sin() / (PI * m)
        };
        let blackman = 0.42 - 0.5 * (2.0 * PI * i as f64 / (len - 1) as f64).cos()
            + 0.08 * (4.0 * PI * i as f64 / (len - 1) as f64).cos();
        taps.push(sinc * blackman);
    }
    let sum: f64 = taps.iter().sum();
    taps.iter().map(|t| t / sum).collect()
}

/// The decimation factor and low-pass cutoff (cycles per full-rate sample) for a band.
fn decimation(window: &Window) -> (usize, f64) {
    let f_max = window.f_max_hz.max(1.0);
    let by_band = (1.0 / (window.dt * 3.2 * f_max)).floor() as usize;
    let by_length = (window.end - window.start) / 150;
    let factor = by_band.min(by_length).max(1);
    (factor, (1.35 * f_max * window.dt).min(0.45))
}

/// The filtered, decimated channels of one window and the poles found in them.
pub struct Fit {
    factor: usize,
    half: usize,
    taps: Vec<f64>,
    /// Full-rate index of each decimated sample.
    first: usize,
    /// Each channel, filtered and decimated (not normalized).
    decimated: Vec<Vec<f64>>,
    /// Poles at the full rate, inside the unit circle.
    pub poles: Vec<Complex64>,
}

impl Fit {
    /// Finds the poles common to `channels` over the window.
    pub fn new(channels: &[&[f64]], window: &Window) -> Option<Fit> {
        let (factor, cutoff) = decimation(window);
        let half = if factor > 1 { 4 * factor } else { 0 };
        let taps = if factor > 1 {
            lowpass(cutoff, half)
        } else {
            vec![1.0]
        };
        // Each output sample uses taps centered on it, so the window leaves `half` samples
        // on either side; the newest `half` samples are not used.
        let first = window.start + half;
        let last = window.end.saturating_sub(half);
        if last <= first + 30 * factor {
            return None;
        }
        let mut decimated: Vec<Vec<f64>> = Vec::new();
        for channel in channels {
            let mut out = Vec::new();
            let mut n = first;
            while n < last {
                let mut acc = 0.0;
                for (k, tap) in taps.iter().enumerate() {
                    acc += tap * channel[n + k - half];
                }
                out.push(acc);
                n += factor;
            }
            decimated.push(out);
        }
        let k = decimated[0].len();
        let pencil = (k / 3).clamp(2, 160);
        let rows_per = k - pencil;
        let mut blocks = Vec::new();
        for signal in &decimated {
            // Scale each channel to unit size so one does not drown the others.
            let norm = signal.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 0.0 {
                blocks.push(signal.iter().map(|x| x / norm).collect::<Vec<f64>>());
            }
        }
        if blocks.is_empty() {
            return None;
        }
        let mut hankel = DMatrix::<f64>::zeros(rows_per * blocks.len(), pencil + 1);
        for (c, signal) in blocks.iter().enumerate() {
            for i in 0..rows_per {
                for j in 0..=pencil {
                    hankel[(c * rows_per + i, j)] = signal[i + j];
                }
            }
        }
        // The right singular vectors of the stacked Hankel matrix, from the eigenvectors of
        // its small Gram matrix (cheaper than an SVD of the tall matrix; the squared
        // singular values kept here stay far above double precision's floor).
        let gram = hankel.tr_mul(&hankel);
        let eigen = gram.symmetric_eigen();
        let mut ranked: Vec<(f64, usize)> = eigen
            .eigenvalues
            .iter()
            .enumerate()
            .map(|(i, value)| (value.max(0.0).sqrt(), i))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let largest = ranked[0].0;
        if largest <= 0.0 {
            return None;
        }
        // Model order: singular values above the float32 field's noise floor.
        let order = ranked
            .iter()
            .filter(|(s, _)| *s > 1e-6 * largest)
            .count()
            .min(pencil - 1)
            .min(140);
        if order == 0 {
            return None;
        }
        let mut v = DMatrix::<f64>::zeros(pencil + 1, order);
        for (column, (_, index)) in ranked.iter().take(order).enumerate() {
            v.set_column(column, &eigen.eigenvectors.column(*index));
        }
        let v1 = v.rows(0, pencil).into_owned();
        let v2 = v.rows(1, pencil).into_owned();
        let a = v1.pseudo_inverse(1e-12).ok()? * v2;
        let mut poles = Vec::new();
        for z in a.complex_eigenvalues().iter() {
            let z = Complex64::new(z.re, z.im);
            let magnitude = z.norm();
            if !(1e-6..0.999_999_9).contains(&magnitude) {
                // Growing or vanishing: not a decay of this system.
                continue;
            }
            // Back to the full rate: the low-pass keeps every pole below the decimated
            // Nyquist, so the principal root is the right one.
            poles.push(Complex64::from_polar(
                magnitude.powf(1.0 / factor as f64),
                z.arg() / factor as f64,
            ));
        }
        if poles.is_empty() {
            return None;
        }
        Some(Fit {
            factor,
            half,
            taps,
            first,
            decimated,
            poles,
        })
    }

    /// The decimated basis `z_i^(factor * r)` and the pseudo-inverse of its Gram matrix,
    /// shared by every channel's amplitude fit.
    fn solver(&self) -> Option<(DMatrix<Complex64>, DMatrix<Complex64>)> {
        let rows = self.decimated[0].len();
        let m = self.poles.len();
        if rows < 2 * m {
            return None;
        }
        let mut basis = DMatrix::<Complex64>::zeros(rows, m);
        for (i, z) in self.poles.iter().enumerate() {
            let stride = z.powu(self.factor as u32);
            let mut power = Complex64::new(1.0, 0.0);
            for r in 0..rows {
                basis[(r, i)] = power;
                power *= stride;
            }
        }
        let gram = basis.ad_mul(&basis);
        let scale = gram.norm();
        let inverse = gram.pseudo_inverse(1e-13 * scale).ok()?;
        Some((basis, inverse))
    }

    /// Channel `index`'s amplitudes `c_i` with `x[n] = sum c_i z_i^(n - first)` at the full
    /// rate, fitted on its filtered, decimated samples and divided by the filter's gain at
    /// each pole (the filter maps `z^n` to `G(z) z^n`).
    fn amplitudes(
        &self,
        index: usize,
        solver: &(DMatrix<Complex64>, DMatrix<Complex64>),
    ) -> Option<Vec<Complex64>> {
        let signal = &self.decimated[index];
        let (basis, inverse) = solver;
        let target = DVector::<Complex64>::from_iterator(
            signal.len(),
            signal.iter().map(|x| Complex64::new(*x, 0.0)),
        );
        let filtered = inverse * basis.ad_mul(&target);
        let mut out = Vec::with_capacity(self.poles.len());
        for (i, z) in self.poles.iter().enumerate() {
            let mut gain = Complex64::new(0.0, 0.0);
            for (k, tap) in self.taps.iter().enumerate() {
                gain += tap * z.powi(k as i32 - self.half as i32);
            }
            out.push(filtered[i] / gain);
        }
        Some(out)
    }

    /// The shared amplitude solver; make it once per fit.
    pub fn shared(&self) -> Option<(DMatrix<Complex64>, DMatrix<Complex64>)> {
        self.solver()
    }

    /// Channel `index`'s Fourier sum at `freqs`: `head` (the recorded samples up to `end`)
    /// plus the fitted free decay from `end` on.
    pub fn extended(
        &self,
        index: usize,
        end: usize,
        dt: f64,
        freqs: &[f64],
        head: &[Complex64],
    ) -> Option<Vec<Complex64>> {
        let solver = self.solver()?;
        self.extended_with(index, end, dt, freqs, head, &solver)
    }

    /// [`Fit::extended`] with the shared solver made once for all channels.
    pub fn extended_with(
        &self,
        index: usize,
        end: usize,
        dt: f64,
        freqs: &[f64],
        head: &[Complex64],
        solver: &(DMatrix<Complex64>, DMatrix<Complex64>),
    ) -> Option<Vec<Complex64>> {
        self.extended_decaying(index, end, dt, freqs, &[], head, solver)
    }

    /// [`Fit::extended_with`] for sums weighted by `e^(-s*t)`, with `s` per frequency in
    /// `decay_per_s` (none when empty).
    #[allow(clippy::too_many_arguments)]
    pub fn extended_decaying(
        &self,
        index: usize,
        end: usize,
        dt: f64,
        freqs: &[f64],
        decay_per_s: &[f64],
        head: &[Complex64],
        solver: &(DMatrix<Complex64>, DMatrix<Complex64>),
    ) -> Option<Vec<Complex64>> {
        let amplitudes = self.amplitudes(index, solver)?;
        let span = (end - self.first) as f64;
        let mut out = Vec::with_capacity(freqs.len());
        for (bin, freq) in freqs.iter().enumerate() {
            let w = 2.0 * PI * freq * dt;
            let s = decay_per_s.get(bin).copied().unwrap_or(0.0) * dt;
            let step = Complex64::from_polar((-s).exp(), -w);
            let at_end = Complex64::from_polar((-s * end as f64).exp(), -w * end as f64);
            let mut tail = Complex64::new(0.0, 0.0);
            for (i, z) in self.poles.iter().enumerate() {
                tail +=
                    amplitudes[i] * z.powf(span) * at_end / (Complex64::new(1.0, 0.0) - z * step);
            }
            out.push(head[bin] + tail);
        }
        Some(out)
    }
}

/// Running Fourier sums `sum x[n] e^(-j w n dt)` of several signals at several frequencies,
/// added to one step at a time.
pub struct RunningSpectra {
    phasor: Vec<Complex64>,
    rotate: Vec<Complex64>,
    /// `s*dt` per frequency: each sum is weighted by `e^(-s*t)`.
    decay: Vec<f64>,
    /// `[channel][frequency]`.
    pub sums: Vec<Vec<Complex64>>,
    count: usize,
}

impl RunningSpectra {
    pub fn new(channels: usize, freqs: &[f64], dt: f64) -> RunningSpectra {
        Self::decaying(channels, freqs, &[], dt)
    }

    /// Sums weighted by `e^(-s*t)`, with `s` per frequency in `decay_per_s` (none when
    /// empty): the spectra of the same system with a uniform loss `s` in its air.
    pub fn decaying(
        channels: usize,
        freqs: &[f64],
        decay_per_s: &[f64],
        dt: f64,
    ) -> RunningSpectra {
        let decay: Vec<f64> = (0..freqs.len())
            .map(|bin| decay_per_s.get(bin).copied().unwrap_or(0.0) * dt)
            .collect();
        RunningSpectra {
            phasor: vec![Complex64::new(1.0, 0.0); freqs.len()],
            rotate: freqs
                .iter()
                .zip(&decay)
                .map(|(f, s)| Complex64::from_polar((-s).exp(), -2.0 * PI * f * dt))
                .collect(),
            decay,
            sums: vec![vec![Complex64::new(0.0, 0.0); freqs.len()]; channels],
            count: 0,
        }
    }

    pub fn push(&mut self, values: &[f64]) {
        for (channel, value) in values.iter().enumerate() {
            for (sum, phasor) in self.sums[channel].iter_mut().zip(&self.phasor) {
                *sum += phasor * value;
            }
        }
        for (phasor, rotate) in self.phasor.iter_mut().zip(&self.rotate) {
            *phasor *= rotate;
        }
        self.count += 1;
        if self.count.is_multiple_of(1024) {
            // Against drift: the phasor's size is known exactly.
            for (phasor, s) in self.phasor.iter_mut().zip(&self.decay) {
                let size = phasor.norm();
                if size > 0.0 {
                    *phasor *= (-s * self.count as f64).exp() / size;
                }
            }
        }
    }
}
