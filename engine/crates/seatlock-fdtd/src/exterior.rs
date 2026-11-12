//! The air outside the cabinet (README section 5.6, `exterior`).
//!
//! The cabinet is a rigid body in a block of air whose outer faces absorb. One run with a
//! simple source at the listening position gives the pressure all over the body's skin, and by
//! reciprocity that is the pressure at the listening position from a small source at each skin
//! point: `p(listener) / U(skin point) = p(skin point) / U(listener)`.
//!
//! The listening position is usually far outside the grid, so the grid holds only the
//! scattered field. The source's own (incident) wave is known in closed form everywhere:
//!
//! ```text
//! p_inc = RHO0 * Q'(t - r/c) / (4*pi*r)
//! v_inc = r_hat * ( Q'(t - r/c) / (4*pi*r*c) + Q(t - r/c) / (4*pi*r^2) )
//! ```
//!
//! for a source of volume velocity `Q(t)`. On the rigid skin the total normal velocity is zero,
//! so the scattered field is driven by the face velocity `-v_inc . n` on every skin face, and
//! the total pressure is the incident pressure (closed form) plus the scattered pressure (grid).
//! `Q(t)` is a Gaussian pulse, so nothing in the drive has a constant part.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

/// A block of cubic cells around a rigid body. Cell `(i, j, k)` has index
/// `(i * ny + j) * nz + k` and its center is at `origin_m + (index + 0.5) * dx`.
#[derive(Clone, Debug)]
pub struct Domain {
    pub n: [usize; 3],
    pub dx: f64,
    pub origin_m: [f64; 3],
    /// True where the cell is air; false inside the body.
    pub air: Vec<bool>,
    /// Middle of the body. The outer faces absorb outgoing spherical waves centered here.
    pub center_m: [f64; 3],
    /// Where the body's surface cuts through cells: how much of each cell and of each face is
    /// air. `None`: every cell is all air or all body (stair-steps).
    pub cut: Option<Cut>,
}

/// Air fractions of a grid that follows the body's surface (README section 5.6).
#[derive(Clone, Debug)]
pub struct Cut {
    /// Air fraction of each cell's volume, never below what keeps the scheme stable.
    pub fill: Vec<f32>,
    /// Air fraction of each face's area, per axis, indexed by the cell on the face's high
    /// side. Zero unless both cells beside the face are air cells.
    pub open: [Vec<f32>; 3],
}

/// Points per side when measuring how much of a cell or a face is air.
const CUT_SAMPLES: usize = 6;
/// A cell with less air than this is taken as body.
const CUT_MIN_FILL: f64 = 0.05;

impl Domain {
    /// A block that reaches `margin_m` past the box `[low_m, high_m]` on every side, with a
    /// cell solid where `solid(center)` holds. Only cells inside the box are asked.
    pub fn around(
        low_m: [f64; 3],
        high_m: [f64; 3],
        margin_m: f64,
        dx: f64,
        solid: &mut dyn FnMut([f64; 3]) -> bool,
    ) -> Self {
        let margin = (margin_m / dx).ceil().max(4.0) as usize;
        let mut n = [0usize; 3];
        let mut origin_m = [0.0; 3];
        for axis in 0..3 {
            let inner = ((high_m[axis] - low_m[axis]) / dx).round().max(1.0) as usize;
            n[axis] = inner + 2 * margin;
            origin_m[axis] = low_m[axis] - margin as f64 * dx;
        }
        let mut domain = Domain {
            n,
            dx,
            origin_m,
            air: vec![true; n[0] * n[1] * n[2]],
            center_m: [
                0.5 * (low_m[0] + high_m[0]),
                0.5 * (low_m[1] + high_m[1]),
                0.5 * (low_m[2] + high_m[2]),
            ],
            cut: None,
        };
        for i in margin..n[0] - margin {
            for j in margin..n[1] - margin {
                for k in margin..n[2] - margin {
                    let cell = domain.cell(i, j, k);
                    if solid(domain.center_of(cell)) {
                        domain.air[cell] = false;
                    }
                }
            }
        }
        domain
    }

    /// [`Domain::around`] with a grid that follows the body's surface: a cell the surface
    /// cuts keeps its air, with the fraction of its volume and of each of its faces that is
    /// air, instead of being all air or all body.
    ///
    /// The scheme is the plain one with three changes. A face's velocity update is scaled by
    /// its air fraction (so the array holds the flow through the face's open part), a cell's
    /// pressure update is divided by its air fraction, and the body's part of a face carries
    /// the prescribed velocity. A cell's fraction is never taken below a sixth of the sum of
    /// its faces' fractions, which keeps the time step of the plain grid stable.
    pub fn around_smooth(
        low_m: [f64; 3],
        high_m: [f64; 3],
        margin_m: f64,
        dx: f64,
        solid: &mut dyn FnMut([f64; 3]) -> bool,
    ) -> Self {
        let mut domain = Self::around(low_m, high_m, margin_m, dx, &mut |_| false);
        let [nx, ny, nz] = domain.n;
        let cells = nx * ny * nz;
        let stride = domain.stride();
        let n = CUT_SAMPLES;
        let margin = (margin_m / dx).ceil().max(4.0) as usize;
        // One layer more than the body's box, for cells its surface only grazes.
        let span = |axis: usize| (margin - 1)..(domain.n[axis] - margin + 1);
        let mut fill = vec![1.0_f32; cells];
        for i in span(0) {
            for j in span(1) {
                for k in span(2) {
                    let cell = domain.cell(i, j, k);
                    let low = [
                        domain.origin_m[0] + i as f64 * dx,
                        domain.origin_m[1] + j as f64 * dx,
                        domain.origin_m[2] + k as f64 * dx,
                    ];
                    let mut inside = 0usize;
                    for a in 0..n {
                        for b in 0..n {
                            for c in 0..n {
                                let p = [
                                    low[0] + (a as f64 + 0.5) / n as f64 * dx,
                                    low[1] + (b as f64 + 0.5) / n as f64 * dx,
                                    low[2] + (c as f64 + 0.5) / n as f64 * dx,
                                ];
                                inside += solid(p) as usize;
                            }
                        }
                    }
                    let air = 1.0 - inside as f64 / (n * n * n) as f64;
                    fill[cell] = air as f32;
                    domain.air[cell] = air >= CUT_MIN_FILL;
                }
            }
        }
        let faces_len = cells + ny * nz;
        let mut open = [
            vec![0.0_f32; faces_len],
            vec![0.0_f32; faces_len],
            vec![0.0_f32; faces_len],
        ];
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let cell = domain.cell(i, j, k);
                    let coords = [i, j, k];
                    for axis in 0..3 {
                        if coords[axis] == 0 {
                            continue;
                        }
                        let below = cell - stride[axis];
                        if !domain.air[cell] || !domain.air[below] {
                            continue;
                        }
                        if fill[cell] >= 1.0 && fill[below] >= 1.0 {
                            open[axis][cell] = 1.0;
                            continue;
                        }
                        // The face on this cell's low side, sampled on its own plane.
                        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                        let mut at = [
                            domain.origin_m[0] + i as f64 * dx,
                            domain.origin_m[1] + j as f64 * dx,
                            domain.origin_m[2] + k as f64 * dx,
                        ];
                        let corner = at;
                        let mut inside = 0usize;
                        for a in 0..n {
                            for b in 0..n {
                                at[u] = corner[u] + (a as f64 + 0.5) / n as f64 * dx;
                                at[v] = corner[v] + (b as f64 + 0.5) / n as f64 * dx;
                                inside += solid(at) as usize;
                            }
                        }
                        open[axis][cell] = 1.0 - inside as f32 / (n * n) as f32;
                    }
                }
            }
        }
        // Keep the plain grid's time step stable in small cells.
        let mut any = false;
        for cell in 0..cells {
            if !domain.air[cell] {
                continue;
            }
            let sum: f32 = (0..3)
                .map(|axis| open[axis][cell] + open[axis][cell + stride[axis]])
                .sum();
            fill[cell] = fill[cell].max(sum / 6.0).max(CUT_MIN_FILL as f32);
            any |= fill[cell] < 1.0 || sum < 6.0 && sum > 0.0 && sum.fract() != 0.0;
        }
        if any {
            domain.cut = Some(Cut { fill, open });
        }
        domain
    }

    pub fn cell(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.n[1] + j) * self.n[2] + k
    }

    pub fn coords(&self, cell: usize) -> [usize; 3] {
        let k = cell % self.n[2];
        let j = (cell / self.n[2]) % self.n[1];
        let i = cell / (self.n[1] * self.n[2]);
        [i, j, k]
    }

    pub fn center_of(&self, cell: usize) -> [f64; 3] {
        let [i, j, k] = self.coords(cell);
        [
            self.origin_m[0] + (i as f64 + 0.5) * self.dx,
            self.origin_m[1] + (j as f64 + 0.5) * self.dx,
            self.origin_m[2] + (k as f64 + 0.5) * self.dx,
        ]
    }

    /// The cell that holds `point_m`, if it is inside the block.
    pub fn cell_at(&self, point_m: [f64; 3]) -> Option<usize> {
        let mut index = [0usize; 3];
        for axis in 0..3 {
            let position = ((point_m[axis] - self.origin_m[axis]) / self.dx).floor();
            if position < 0.0 || position >= self.n[axis] as f64 {
                return None;
            }
            index[axis] = position as usize;
        }
        Some(self.cell(index[0], index[1], index[2]))
    }

    pub fn time_step_s(&self) -> f64 {
        0.95 * self.dx / (C0 * 3.0_f64.sqrt())
    }

    /// Highest frequency the grid resolves, `C0 / (10 * dx)` (README section 5.5).
    pub fn valid_up_to_hz(&self) -> f64 {
        C0 / (10.0 * self.dx)
    }

    fn stride(&self) -> [usize; 3] {
        [self.n[1] * self.n[2], self.n[2], 1]
    }

    /// The solid neighbors of an air cell: `(axis, true when the neighbor is on the low side)`.
    pub fn solid_neighbors(&self, cell: usize) -> Vec<(u8, bool)> {
        let mut found = Vec::new();
        self.each_solid_neighbor(cell, &mut |axis, low_side| found.push((axis, low_side)));
        found
    }

    fn each_solid_neighbor(&self, cell: usize, visit: &mut dyn FnMut(u8, bool)) {
        let coords = self.coords(cell);
        let stride = self.stride();
        for axis in 0..3 {
            if coords[axis] > 0 && !self.air[cell - stride[axis]] {
                visit(axis as u8, true);
            }
            if coords[axis] + 1 < self.n[axis] && !self.air[cell + stride[axis]] {
                visit(axis as u8, false);
            }
        }
    }

    /// Air cells that touch the body.
    pub fn skin_cells(&self) -> Vec<usize> {
        (0..self.air.len())
            .filter(|&cell| {
                let mut touches = false;
                if self.air[cell] {
                    self.each_solid_neighbor(cell, &mut |_, _| touches = true);
                }
                touches
            })
            .collect()
    }
}

/// The wave that arrives from the listening position.
#[derive(Clone, Copy, Debug)]
pub enum Incident {
    /// A simple source at a point, inside or outside the block.
    Point { at_m: [f64; 3] },
    /// A simple source so far away, in the unit direction `toward` from `reference_m`, that
    /// its wave is plane at the body. `distance_m` sets its level and phase.
    Plane {
        toward: [f64; 3],
        distance_m: f64,
        reference_m: [f64; 3],
    },
}

impl Incident {
    /// Travel distance of the wave to `point`, and its direction of travel there.
    fn path(&self, point: [f64; 3]) -> (f64, [f64; 3]) {
        match *self {
            Incident::Point { at_m } => {
                let offset = [point[0] - at_m[0], point[1] - at_m[1], point[2] - at_m[2]];
                let r = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2])
                    .sqrt()
                    .max(1e-9);
                (r, [offset[0] / r, offset[1] / r, offset[2] / r])
            }
            Incident::Plane {
                toward,
                distance_m,
                reference_m,
            } => {
                let along = toward[0] * (point[0] - reference_m[0])
                    + toward[1] * (point[1] - reference_m[1])
                    + toward[2] * (point[2] - reference_m[2]);
                (distance_m - along, [-toward[0], -toward[1], -toward[2]])
            }
        }
    }

    /// Spreading distance: the travel distance for a point source, the nominal distance for a
    /// plane wave.
    fn spread(&self, travel: f64) -> f64 {
        match *self {
            Incident::Point { .. } => travel,
            Incident::Plane { distance_m, .. } => distance_m,
        }
    }

    fn is_plane(&self) -> bool {
        matches!(self, Incident::Plane { .. })
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub frequencies_hz: Vec<f64>,
    /// Steps between progress calls; 0 for none.
    pub progress_every: usize,
}

#[derive(Clone, Debug, Default)]
pub struct SkinTransfer {
    /// `[recorded cell][frequency]`: pressure at the cell per unit volume velocity of the
    /// source. By reciprocity it is also the pressure at the source's position per unit volume
    /// velocity of a small source on the skin at that cell.
    pub transfer: Vec<Vec<Complex64>>,
    pub steps: usize,
    pub max_steps: usize,
    pub decayed: bool,
    pub canceled: bool,
    pub elapsed_ms: f64,
}

/// One face whose velocity is prescribed: `v = a * g'(t - delay) + b * g(t - delay)`, with `g`
/// the Gaussian pulse.
struct Driven {
    axis: u8,
    slot: usize,
    delay_s: f64,
    a: f32,
    b: f32,
    /// 0 for a face between the body and the air, whose whole velocity is prescribed.
    /// Otherwise the face lies between two air cells and this is the body's share of it:
    /// that share carries the prescribed velocity, beside the flow through the open part.
    share: f32,
}

struct Pulse {
    sigma: f64,
    t0: f64,
}

impl Pulse {
    fn new(dt: f64) -> Self {
        let sigma = 8.0 * dt;
        Pulse {
            sigma,
            t0: 4.0 * sigma,
        }
    }

    /// `g(t)` and `g'(t)`.
    fn at(&self, t: f64) -> (f64, f64) {
        let x = (t - self.t0) / self.sigma;
        if x.abs() > 5.5 {
            return (0.0, 0.0);
        }
        let g = (-x * x).exp();
        (g, -2.0 * x / self.sigma * g)
    }

    /// Fourier transform of `g`: `sigma * sqrt(pi) * exp(-(pi*f*sigma)^2) * e^(-j*omega*t0)`.
    fn spectrum(&self, freq_hz: f64) -> Complex64 {
        let magnitude = self.sigma * PI.sqrt() * (-(PI * freq_hz * self.sigma).powi(2)).exp();
        Complex64::from_polar(magnitude, -2.0 * PI * freq_hz * self.t0)
    }
}

struct Raw {
    samples: Vec<Vec<f32>>,
    steps: usize,
    max_steps: usize,
    decayed: bool,
    canceled: bool,
}

/// Steps the scattered (or radiated) field on the block. `latest_delay_s` is when the last
/// driven face stops; the run then goes on until the recorded cells are quiet.
fn run_driven(
    domain: &Domain,
    driven: &[Driven],
    record: &[usize],
    latest_delay_s: f64,
    progress_every: usize,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Raw {
    let [nx, ny, nz] = domain.n;
    let cells = nx * ny * nz;
    let dx = domain.dx;
    let dt = domain.time_step_s();
    let kv = (dt / (RHO0 * dx)) as f32;
    let kp = RHO0 * C0 * C0 * dt / dx;
    let kp32 = kp as f32;
    let stride = domain.stride();
    let faces_len = cells + ny * nz;
    let pulse = Pulse::new(dt);

    // Faces between two air cells, as a 0/1 mask per axis.
    let mut open: [Vec<f32>; 3] = [
        vec![0.0; faces_len],
        vec![0.0; faces_len],
        vec![0.0; faces_len],
    ];
    // The outermost layer of cells absorbs an outgoing spherical wave centered on the body's
    // middle: `dp/dt + (c/cos) * dp/dn + (c/r) * p = 0` along the inward-pointing axis `n`
    // (first-order Bayliss-Turkel, in Mur's box form between the cell and its inward
    // neighbor). `cos = r_hat . n` is the angle at which that wave crosses the layer.
    struct Outer {
        cell: usize,
        inward: usize,
        /// How many outer faces the cell has: faces first, then edges, then corners, so that
        /// an edge cell's inward neighbor (a face cell) already has its new value.
        rank: u8,
        nu: f64,
        beta: f64,
        old: f32,
        old_inward: f32,
    }
    let courant = C0 * dt / dx;
    let mut outers: Vec<Outer> = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = domain.cell(i, j, k);
                if !domain.air[c] {
                    continue;
                }
                let coords = [i, j, k];
                let center = domain.center_of(c);
                let from_middle = [
                    center[0] - domain.center_m[0],
                    center[1] - domain.center_m[1],
                    center[2] - domain.center_m[2],
                ];
                let r = (from_middle[0].powi(2) + from_middle[1].powi(2) + from_middle[2].powi(2))
                    .sqrt()
                    .max(2.0 * dx);
                let mut rank = 0u8;
                let mut best: Option<(f64, usize)> = None;
                for axis in 0..3 {
                    if coords[axis] > 0 && domain.air[c - stride[axis]] {
                        open[axis][c] = match &domain.cut {
                            Some(cut) => cut.open[axis][c],
                            None => 1.0,
                        };
                    }
                    let low = coords[axis] == 0;
                    let high = coords[axis] + 1 == domain.n[axis];
                    if low || high {
                        rank += 1;
                        let cos = from_middle[axis].abs() / r;
                        let inward = if low {
                            c + stride[axis]
                        } else {
                            c - stride[axis]
                        };
                        if best.is_none_or(|(most, _)| cos > most) {
                            best = Some((cos, inward));
                        }
                    }
                }
                if let Some((cos, inward)) = best {
                    outers.push(Outer {
                        cell: c,
                        inward,
                        rank,
                        // Keep the box scheme's speed under one cell per step.
                        nu: courant / cos.max(0.58),
                        beta: 0.5 * C0 * dt / r,
                        old: 0.0,
                        old_inward: 0.0,
                    });
                }
            }
        }
    }
    outers.sort_by_key(|outer| outer.rank);

    let diagonal = dx * ((nx * nx + ny * ny + nz * nz) as f64).sqrt();
    let pulse_end = pulse.t0 + 5.5 * pulse.sigma + latest_delay_s;
    let earliest_stop = pulse_end + diagonal / C0;
    let max_steps = ((pulse_end + 8.0 * diagonal / C0) / dt).ceil() as usize;
    let quiet_needed = 200usize.max((0.001 / dt) as usize);

    // With cut cells: the pressure update of each cell divided by its air fraction.
    let kp_cell: Option<Vec<f32>> = domain.cut.as_ref().map(|cut| {
        cut.fill
            .iter()
            .map(|fill| kp32 / fill.max(CUT_MIN_FILL as f32))
            .collect()
    });
    let whole: Vec<&Driven> = driven.iter().filter(|face| face.share == 0.0).collect();
    let shared: Vec<&Driven> = driven.iter().filter(|face| face.share > 0.0).collect();

    let mut pressure = vec![0.0_f32; cells];
    let mut v = [
        vec![0.0_f32; faces_len],
        vec![0.0_f32; faces_len],
        vec![0.0_f32; faces_len],
    ];
    let mut samples: Vec<Vec<f32>> = vec![Vec::new(); record.len()];
    let (sx, sy) = (stride[0], stride[1]);
    let mut peak = 0.0_f32;
    let mut quiet_run = 0usize;
    let mut decayed = false;
    let mut canceled = false;
    let mut steps = 0usize;

    while steps < max_steps {
        let half = (steps as f64 + 0.5) * dt;
        for axis in 0..3 {
            let s = stride[axis];
            let mask = &open[axis];
            let vel = &mut v[axis];
            for c in s..cells {
                vel[c] -= kv * mask[c] * (pressure[c] - pressure[c - s]);
            }
        }
        if half < pulse_end {
            for face in &whole {
                let (g, slope) = pulse.at(half - face.delay_s);
                v[face.axis as usize][face.slot] = face.a * slope as f32 + face.b * g as f32;
            }
        } else {
            for face in &whole {
                v[face.axis as usize][face.slot] = 0.0;
            }
        }
        let [vx, vy, vz] = &v;
        let net = |c: usize| -> f32 { vx[c + sx] - vx[c] + vy[c + sy] - vy[c] + vz[c + 1] - vz[c] };
        for outer in outers.iter_mut() {
            outer.old = pressure[outer.cell];
            outer.old_inward = pressure[outer.inward];
        }
        // Body cells have only closed or driven faces; their pressure is never read.
        match &kp_cell {
            None => {
                for (c, p) in pressure.iter_mut().enumerate() {
                    *p -= kp32 * net(c);
                }
            }
            Some(kp_cell) => {
                for (c, p) in pressure.iter_mut().enumerate() {
                    *p -= kp_cell[c] * net(c);
                }
                // The body's share of a face between two air cells: its flow leaves the cell
                // on the face's low side and enters the one on its high side.
                if half < pulse_end {
                    for face in &shared {
                        let (g, slope) = pulse.at(half - face.delay_s);
                        let flow = face.share * (face.a * slope as f32 + face.b * g as f32);
                        let high = face.slot;
                        let low = high - stride[face.axis as usize];
                        pressure[high] += kp_cell[high] * flow;
                        pressure[low] -= kp_cell[low] * flow;
                    }
                }
            }
        }
        for outer in &outers {
            let (nu, beta) = (outer.nu, outer.beta);
            let updated = (outer.old as f64 * (1.0 - nu - beta)
                + pressure[outer.inward] as f64 * (nu - 1.0 - beta)
                + outer.old_inward as f64 * (1.0 + nu - beta))
                / (1.0 + nu + beta);
            pressure[outer.cell] = updated as f32;
        }
        let mut loudest = 0.0_f32;
        for (slot, &cell) in record.iter().enumerate() {
            let value = pressure[cell];
            samples[slot].push(value);
            loudest = loudest.max(value.abs());
        }
        peak = peak.max(loudest);
        steps += 1;
        let time = steps as f64 * dt;
        if time > earliest_stop {
            if loudest <= 1e-4 * peak {
                quiet_run += 1;
            } else {
                quiet_run = 0;
            }
            if quiet_run >= quiet_needed {
                decayed = true;
                break;
            }
        }
        if progress_every > 0 && steps.is_multiple_of(progress_every) && !progress(steps, max_steps)
        {
            canceled = true;
            break;
        }
    }
    Raw {
        samples,
        steps,
        max_steps,
        decayed,
        canceled,
    }
}

/// `sum x[m] * e^(-j*omega*(m+1)*dt) * dt`: the recorded pressure after step `m` is at time
/// `(m+1)*dt`.
fn spectrum(samples: &[f32], freq_hz: f64, dt: f64) -> Complex64 {
    let step = Complex64::from_polar(1.0, -2.0 * PI * freq_hz * dt);
    let mut phasor = step;
    let mut sum = Complex64::new(0.0, 0.0);
    for (index, &x) in samples.iter().enumerate() {
        sum += phasor * x as f64;
        phasor *= step;
        if index % 1024 == 1023 {
            phasor /= phasor.norm();
        }
    }
    sum * dt
}

/// Pressure at each recorded cell per unit volume velocity of the incident wave's source, with
/// the rigid body in place.
pub fn skin_transfer(
    domain: &Domain,
    incident: &Incident,
    record: &[usize],
    options: &Options,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> SkinTransfer {
    let started = crate::clock_start();
    let dx = domain.dx;
    let stride = domain.stride();
    // Every face between the body and the air carries minus the incident wave's velocity,
    // and so does the body's share of a face that the surface cuts.
    let mut faces: Vec<(u8, usize, f64, [f64; 3], f32)> = Vec::new();
    let mut nearest = f64::MAX;
    for cell in 0..domain.air.len() {
        if !domain.air[cell] {
            continue;
        }
        domain.each_solid_neighbor(cell, &mut |axis, low_side| {
            let mut at = domain.center_of(cell);
            at[axis as usize] += if low_side { -0.5 * dx } else { 0.5 * dx };
            let slot = if low_side {
                cell
            } else {
                cell + stride[axis as usize]
            };
            let (travel, direction) = incident.path(at);
            nearest = nearest.min(travel);
            faces.push((axis, slot, travel, direction, 0.0));
        });
        if let Some(cut) = &domain.cut {
            let coords = domain.coords(cell);
            for axis in 0..3 {
                if coords[axis] == 0 || !domain.air[cell - stride[axis]] {
                    continue;
                }
                let share = 1.0 - cut.open[axis][cell];
                if share <= 0.0 {
                    continue;
                }
                let mut at = domain.center_of(cell);
                at[axis] -= 0.5 * dx;
                let (travel, direction) = incident.path(at);
                nearest = nearest.min(travel);
                faces.push((axis as u8, cell, travel, direction, share));
            }
        }
    }
    if faces.is_empty() {
        nearest = 0.0;
    }
    let mut latest = 0.0_f64;
    let driven: Vec<Driven> = faces
        .iter()
        .map(|&(axis, slot, travel, direction, share)| {
            let spread = incident.spread(travel);
            let along = direction[axis as usize];
            let delay_s = (travel - nearest) / C0;
            latest = latest.max(delay_s);
            Driven {
                axis,
                slot,
                delay_s,
                a: (-along / (4.0 * PI * spread * C0)) as f32,
                b: if incident.is_plane() {
                    0.0
                } else {
                    (-along / (4.0 * PI * spread * spread)) as f32
                },
                share,
            }
        })
        .collect();
    let raw = run_driven(
        domain,
        &driven,
        record,
        latest,
        options.progress_every,
        progress,
    );
    let dt = domain.time_step_s();
    let pulse = Pulse::new(dt);
    let transfer = record
        .iter()
        .zip(&raw.samples)
        .map(|(&cell, samples)| {
            let (travel, _) = incident.path(domain.center_of(cell));
            let spread = incident.spread(travel);
            options
                .frequencies_hz
                .iter()
                .map(|&freq| {
                    let omega = 2.0 * PI * freq;
                    let k = omega / C0;
                    // The run's clock starts when the wave reaches the nearest skin face.
                    let scattered = spectrum(samples, freq, dt) / pulse.spectrum(freq)
                        * Complex64::from_polar(1.0, -k * nearest);
                    let incident_wave = Complex64::new(0.0, omega * RHO0)
                        * Complex64::from_polar(1.0 / (4.0 * PI * spread), -k * travel);
                    incident_wave + scattered
                })
                .collect()
        })
        .collect();
    SkinTransfer {
        transfer,
        steps: raw.steps,
        max_steps: raw.max_steps,
        decayed: raw.decayed,
        canceled: raw.canceled,
        elapsed_ms: crate::elapsed_ms(started),
    }
}

/// The other way round, for checking reciprocity: one skin face of `cell` (toward its solid
/// neighbor on `axis`) moves as a source, and the pressure is recorded at `record`. Returns the
/// pressure at each recorded cell per unit volume velocity of the face.
pub fn radiate_from_face(
    domain: &Domain,
    cell: usize,
    axis: u8,
    low_side: bool,
    record: &[usize],
    options: &Options,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> SkinTransfer {
    let started = crate::clock_start();
    let stride = domain.stride();
    let slot = if low_side {
        cell
    } else {
        cell + stride[axis as usize]
    };
    // Positive volume velocity pushes air away from the body.
    let sign = if low_side { 1.0 } else { -1.0 };
    let driven = [Driven {
        axis,
        slot,
        delay_s: 0.0,
        a: 0.0,
        b: (sign / (domain.dx * domain.dx)) as f32,
        share: 0.0,
    }];
    let raw = run_driven(
        domain,
        &driven,
        record,
        0.0,
        options.progress_every,
        progress,
    );
    let dt = domain.time_step_s();
    let pulse = Pulse::new(dt);
    let transfer = raw
        .samples
        .iter()
        .map(|samples| {
            options
                .frequencies_hz
                .iter()
                .map(|&freq| spectrum(samples, freq, dt) / pulse.spectrum(freq))
                .collect()
        })
        .collect();
    SkinTransfer {
        transfer,
        steps: raw.steps,
        max_steps: raw.max_steps,
        decayed: raw.decayed,
        canceled: raw.canceled,
        elapsed_ms: crate::elapsed_ms(started),
    }
}
