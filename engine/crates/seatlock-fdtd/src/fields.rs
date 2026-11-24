//! Pressure-field capture for the 3D wave views (README section 12, Level 3 D):
//! standing-wave patterns at chosen frequencies, and a short pulse animation.

use serde::Serialize;

/// What to record while the grid runs.
#[derive(Clone, Debug)]
pub struct FieldCapture {
    /// Frequencies for complex pressure at every kept air cell.
    pub standing_hz: Vec<f64>,
    /// Keep a pressure frame every this many steps (0 = no pulse).
    pub pulse_every: usize,
    pub pulse_max_frames: usize,
    /// Keep every Nth cell along each axis (1 = all air cells).
    pub stride: usize,
}

impl Default for FieldCapture {
    fn default() -> Self {
        Self {
            standing_hz: Vec::new(),
            pulse_every: 0,
            pulse_max_frames: 0,
            stride: 1,
        }
    }
}

/// Sparse air cells and the pressures recorded on them.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CapturedField {
    pub dx_m: f64,
    pub origin_m: [f64; 3],
    pub n: [usize; 3],
    /// Cell indices (into the full grid) that were kept.
    pub cells: Vec<u32>,
    /// Cell-centre positions in metres, length `3 * cells.len()`.
    pub positions_m: Vec<f32>,
    pub standing: Vec<StandingBand>,
    pub pulse: Option<PulseFrames>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StandingBand {
    pub frequency_hz: f64,
    pub re: Vec<f32>,
    pub im: Vec<f32>,
    pub max_abs: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PulseFrames {
    pub dt_s: f64,
    pub times_s: Vec<f32>,
    /// Each frame is pressure at the kept cells.
    pub frames: Vec<Vec<f32>>,
    pub max_abs: f32,
}

/// Accumulator used inside the time loop.
pub struct FieldAccumulator {
    pub cells: Vec<usize>,
    pub positions_m: Vec<f32>,
    standing_hz: Vec<f64>,
    /// Running DFT: `standing_re/im[band][kept]`.
    standing_re: Vec<Vec<f64>>,
    standing_im: Vec<Vec<f64>>,
    /// Rotating phasor per band.
    phase_re: Vec<f64>,
    phase_im: Vec<f64>,
    phase_step_re: Vec<f64>,
    phase_step_im: Vec<f64>,
    pulse_every: usize,
    pulse_max_frames: usize,
    times_s: Vec<f32>,
    frames: Vec<Vec<f32>>,
    pulse_peak: f32,
    dt: f64,
    dx: f64,
    origin: [f64; 3],
    n: [usize; 3],
}

impl FieldAccumulator {
    pub fn new(grid: &super::Grid, capture: &FieldCapture, dt: f64) -> Self {
        let stride = capture.stride.max(1);
        let [nx, ny, nz] = grid.n;
        let mut cells = Vec::new();
        let mut positions_m = Vec::new();
        for i in (0..nx).step_by(stride) {
            for j in (0..ny).step_by(stride) {
                for k in (0..nz).step_by(stride) {
                    let c = grid.cell(i, j, k);
                    if !grid.air[c] {
                        continue;
                    }
                    cells.push(c);
                    positions_m.push((grid.origin_m[0] + (i as f64 + 0.5) * grid.dx) as f32);
                    positions_m.push((grid.origin_m[1] + (j as f64 + 0.5) * grid.dx) as f32);
                    positions_m.push((grid.origin_m[2] + (k as f64 + 0.5) * grid.dx) as f32);
                }
            }
        }
        let n_kept = cells.len();
        let bands = capture.standing_hz.len();
        let mut phase_step_re = Vec::with_capacity(bands);
        let mut phase_step_im = Vec::with_capacity(bands);
        for &freq in &capture.standing_hz {
            let step = -2.0 * std::f64::consts::PI * freq * dt;
            phase_step_re.push(step.cos());
            phase_step_im.push(step.sin());
        }
        Self {
            cells,
            positions_m,
            standing_hz: capture.standing_hz.clone(),
            standing_re: vec![vec![0.0; n_kept]; bands],
            standing_im: vec![vec![0.0; n_kept]; bands],
            phase_re: vec![1.0; bands],
            phase_im: vec![0.0; bands],
            phase_step_re,
            phase_step_im,
            pulse_every: capture.pulse_every,
            pulse_max_frames: capture.pulse_max_frames,
            times_s: Vec::new(),
            frames: Vec::new(),
            pulse_peak: 0.0,
            dt,
            dx: grid.dx,
            origin: grid.origin_m,
            n: grid.n,
        }
    }

    /// Call once per time step after the pressure update, with `time = steps * dt`.
    pub fn step(&mut self, pressure: &[f32], steps: usize, time: f64) {
        let n_kept = self.cells.len();
        for band in 0..self.standing_hz.len() {
            let wr = self.phase_re[band];
            let wi = self.phase_im[band];
            let re = &mut self.standing_re[band];
            let im = &mut self.standing_im[band];
            for (kept, &cell) in self.cells.iter().enumerate() {
                let p = pressure[cell] as f64;
                re[kept] += p * wr;
                im[kept] += p * wi;
            }
            let next_r = wr * self.phase_step_re[band] - wi * self.phase_step_im[band];
            let next_i = wr * self.phase_step_im[band] + wi * self.phase_step_re[band];
            self.phase_re[band] = next_r;
            self.phase_im[band] = next_i;
            if steps % 1024 == 1023 {
                let norm = (self.phase_re[band].hypot(self.phase_im[band])).max(1e-30);
                self.phase_re[band] /= norm;
                self.phase_im[band] /= norm;
            }
        }
        if self.pulse_every > 0
            && self.frames.len() < self.pulse_max_frames
            && steps.is_multiple_of(self.pulse_every)
        {
            let mut frame = Vec::with_capacity(n_kept);
            let mut peak = self.pulse_peak;
            for &cell in &self.cells {
                let p = pressure[cell];
                peak = peak.max(p.abs());
                frame.push(p);
            }
            self.pulse_peak = peak;
            self.times_s.push(time as f32);
            self.frames.push(frame);
        }
    }

    pub fn finish(self) -> CapturedField {
        let mut standing = Vec::with_capacity(self.standing_hz.len());
        for (band, &freq) in self.standing_hz.iter().enumerate() {
            let mut re = Vec::with_capacity(self.cells.len());
            let mut im = Vec::with_capacity(self.cells.len());
            let mut max_abs = 0.0_f32;
            for kept in 0..self.cells.len() {
                let r = (self.standing_re[band][kept] * self.dt) as f32;
                let i = (self.standing_im[band][kept] * self.dt) as f32;
                max_abs = max_abs.max((r * r + i * i).sqrt());
                re.push(r);
                im.push(i);
            }
            standing.push(StandingBand {
                frequency_hz: freq,
                re,
                im,
                max_abs,
            });
        }
        let pulse = if self.frames.is_empty() {
            None
        } else {
            Some(PulseFrames {
                dt_s: self.dt * self.pulse_every.max(1) as f64,
                times_s: self.times_s,
                frames: self.frames,
                max_abs: self.pulse_peak,
            })
        };
        CapturedField {
            dx_m: self.dx,
            origin_m: self.origin,
            n: self.n,
            cells: self.cells.into_iter().map(|c| c as u32).collect(),
            positions_m: self.positions_m,
            standing,
            pulse,
        }
    }
}
