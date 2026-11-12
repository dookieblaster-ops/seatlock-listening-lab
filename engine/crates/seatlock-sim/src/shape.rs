//! Shaping the inside (README section 5.5): the 3D model works out, cell by cell, where
//! plastic inside the cabinet evens out the sound, from the gradient of
//! [`seatlock_fdtd::adjoint`].
//!
//! The objective is the unevenness of the level across a band: the root-mean-square
//! difference, in dB, between the level at evenly spaced frequencies and its mean. The level
//! is that of the driver's and the openings' summed volume velocity,
//! `U_d * (1 - sum of H_k)`, so the baffle and the listening distance, which the inside does
//! not change, are left out. The 3D run gives its air a uniform loss (`decay_per_s`), which
//! keeps every run short and every resonance of finite height.
//!
//! Each cell of the design region has a density from 0 (plastic) to 1 (air); a face is open
//! by the product of the densities on its two sides. Densities come from the design
//! variables through a smoothing filter and a projection that sharpens step by step, the
//! usual way of topology optimization (Bokhari, Berggren, Noreland, Wadbro, Scientific
//! Reports 13:21248, 2023).

use num_complex::Complex64;
use seatlock_core::design::DesignFile;
use seatlock_core::registry::{Radiator, Registry};
use seatlock_fdtd::adjoint::{self, Face, Problem};
use seatlock_fdtd::Grid;
use serde::{Deserialize, Serialize};

use crate::voxels::CabinetGrid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeRequest {
    /// Cell size of the 3D grid.
    pub dx_m: f64,
    /// The box in which plastic may be placed, in cabinet coordinates. Without one, all the
    /// chamber air.
    pub region_min_m: Option<[f64; 3]>,
    pub region_max_m: Option<[f64; 3]>,
    /// The band whose level is evened out.
    pub band_hz: [f64; 2],
    /// The uniform loss given to the air, as a decay rate per second: every resonance is
    /// widened by this over pi, in Hz.
    pub decay_per_s: f64,
    /// The most plastic that may be added, as a fraction of the region.
    pub max_plastic_fraction: f64,
    /// Steps of the optimizer.
    pub iterations: usize,
    /// Air kept free around the cone's back.
    pub keep_clear_m: f64,
}

impl Default for ShapeRequest {
    fn default() -> Self {
        ShapeRequest {
            dx_m: 0.01,
            region_min_m: None,
            region_max_m: None,
            band_hz: [150.0, 1500.0],
            decay_per_s: 100.0,
            max_plastic_fraction: 0.15,
            iterations: 40,
            keep_clear_m: 0.03,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShapeResult {
    pub dx_m: f64,
    /// The low corner of the box of cells `solid` describes, and its size in cells. The cell
    /// `(i, j, k)` is at `(i * size[1] + j) * size[2] + k`.
    pub min_m: [f64; 3],
    pub size: [usize; 3],
    /// True where the shaped inside is plastic, with the wall cells that plastic touches
    /// (the `voxels` element counts only what lies in the air).
    pub solid: Vec<bool>,
    pub plastic_m3: f64,
    pub region_m3: f64,
    pub frequencies_hz: Vec<f64>,
    /// Level across the band, mean removed, for the plain cabinet and for the shaped one.
    pub level_before_db: Vec<f64>,
    pub level_after_db: Vec<f64>,
    /// Unevenness across the band (RMS, dB): the plain cabinet; the shaped one while its
    /// cells could still be part air; and the final plastic-or-air shape, cleaned for
    /// printing and run again from scratch.
    pub uneven_before_db: f64,
    pub uneven_shaped_db: f64,
    pub uneven_after_db: f64,
    /// Unevenness after each accepted step.
    pub history_db: Vec<f64>,
    pub runs: usize,
    pub steps_per_run: usize,
    pub canceled: bool,
    pub notes: Vec<String>,
}

/// The objective and its gradient for one cabinet.
pub struct Shaper {
    grid: Grid,
    /// The grid cell of each design variable.
    cells: Vec<usize>,
    /// The variable of each grid cell, or `u32::MAX`.
    var_of: Vec<u32>,
    faces: Vec<Face>,
    /// The variables at the low and the high side of each face (`u32::MAX`: fixed air).
    ends: Vec<(u32, u32)>,
    /// Neighbors of each variable within the filter's radius, with weights that sum to 1.
    near: Vec<Vec<(u32, f64)>>,
    freqs: Vec<f64>,
    radiator: Box<dyn Radiator>,
    decay_per_s: f64,
    max_steps: usize,
    pub notes: Vec<String>,
    pub runs: usize,
    pub steps_per_run: usize,
}

pub struct Evaluation {
    /// Mean squared difference from the band's mean level, dB^2.
    pub objective: f64,
    /// Level at each frequency, mean removed.
    pub level_db: Vec<f64>,
    /// `d objective / d density` per variable (empty when not asked for).
    pub gradient: Vec<f64>,
}

const FILTER_RADIUS_CELLS: f64 = 1.5;
const QUIET: f64 = 1e-5;

impl Shaper {
    pub fn new(
        design: &DesignFile,
        request: &ShapeRequest,
        registry: &Registry,
    ) -> Result<Shaper, String> {
        if !(0.002..=0.05).contains(&request.dx_m) {
            return Err("the cell must be between 2 and 50 mm.".into());
        }
        if request.decay_per_s <= 0.0 {
            return Err("the air's loss must be above zero.".into());
        }
        let outer = registry.create_outer(
            &design.speaker.enclosure.outer.kind,
            &design.speaker.enclosure.outer.fields,
        )?;
        let built: CabinetGrid = crate::cabinet_grid_of(design, outer.as_ref(), request.dx_m)?;
        let part = design
            .speaker
            .radiators
            .iter()
            .find(|part| part.kind == "cone" && crate::driver_loads_box(part))
            .ok_or("no driver opens into the cabinet's air.")?;
        let radiator = registry.create_radiator(&part.kind, &part.fields)?;
        let grid = built.grid;
        let [_, ny, nz] = grid.n;
        let dx = grid.dx;
        let stride = [ny * nz, nz, 1usize];
        let coords = |cell: usize| [cell / (ny * nz), (cell / nz) % ny, cell % nz];
        let center = |cell: usize| -> [f64; 3] {
            let c = coords(cell);
            std::array::from_fn(|axis| grid.origin_m[axis] + (c[axis] as f64 + 0.5) * dx)
        };

        // Cells that stay air: the source, the air near the cone, and both sides of every
        // opening's plane.
        let mut fixed = vec![false; grid.air.len()];
        for &cell in &grid.source {
            fixed[cell] = true;
        }
        for opening in &grid.openings {
            for &(axis, slot, _) in opening {
                fixed[slot] = true;
                fixed[slot - stride[axis as usize]] = true;
            }
        }
        let sources: Vec<[f64; 3]> = grid.source.iter().map(|cell| center(*cell)).collect();
        let mut cells = Vec::new();
        let mut var_of = vec![u32::MAX; grid.air.len()];
        for cell in 0..grid.air.len() {
            if !built.chamber_air[cell] || fixed[cell] {
                continue;
            }
            let p = center(cell);
            let inside = (0..3).all(|axis| {
                request.region_min_m.is_none_or(|lo| p[axis] >= lo[axis])
                    && request.region_max_m.is_none_or(|hi| p[axis] <= hi[axis])
            });
            let clear = sources.iter().all(|s| {
                (0..3).map(|axis| (p[axis] - s[axis]).powi(2)).sum::<f64>()
                    > request.keep_clear_m * request.keep_clear_m
            });
            if inside && clear {
                var_of[cell] = cells.len() as u32;
                cells.push(cell);
            }
        }
        if cells.is_empty() {
            return Err("there is no free air in the region to shape.".into());
        }
        let mut faces = Vec::new();
        let mut ends = Vec::new();
        for &cell in &cells {
            let c = coords(cell);
            for axis in 0..3 {
                let s = stride[axis];
                if c[axis] > 0 && grid.air[cell - s] {
                    faces.push((axis as u8, cell));
                    ends.push((var_of[cell - s], var_of[cell]));
                }
                if c[axis] + 1 < grid.n[axis] && grid.air[cell + s] && var_of[cell + s] == u32::MAX
                {
                    faces.push((axis as u8, cell + s));
                    ends.push((var_of[cell], u32::MAX));
                }
            }
        }
        let reach = FILTER_RADIUS_CELLS.ceil() as i64;
        let mut near = Vec::with_capacity(cells.len());
        for &cell in &cells {
            let c = coords(cell);
            let mut list = Vec::new();
            for di in -reach..=reach {
                for dj in -reach..=reach {
                    for dk in -reach..=reach {
                        let distance = ((di * di + dj * dj + dk * dk) as f64).sqrt();
                        let at = [c[0] as i64 + di, c[1] as i64 + dj, c[2] as i64 + dk];
                        if distance >= FILTER_RADIUS_CELLS
                            || (0..3).any(|a| at[a] < 0 || at[a] >= grid.n[a] as i64)
                        {
                            continue;
                        }
                        let other = (at[0] as usize * ny + at[1] as usize) * nz + at[2] as usize;
                        if var_of[other] != u32::MAX {
                            list.push((var_of[other], FILTER_RADIUS_CELLS - distance));
                        }
                    }
                }
            }
            let total: f64 = list.iter().map(|(_, w)| w).sum();
            for entry in &mut list {
                entry.1 /= total;
            }
            near.push(list);
        }
        // Frequencies no farther apart than a resonance is wide with this loss, so that none
        // can hide between two of them.
        let [low, high] = request.band_hz;
        let valid = seatlock_core::constants::C0 / (10.0 * dx);
        let high = high.min(valid);
        if !(low > 0.0 && high > low) {
            return Err("the band is empty on this grid.".into());
        }
        let spacing = request.decay_per_s / std::f64::consts::PI;
        let count = (((high - low) / spacing).ceil() as usize).clamp(2, 96);
        let freqs: Vec<f64> = (0..=count)
            .map(|index| low + (high - low) * index as f64 / count as f64)
            .collect();
        let dt = grid.time_step_s();
        let mut notes = built.notes;
        notes.push(format!(
            "The air is given a uniform loss while the inside is shaped: every resonance is \
             {:.0} Hz wide. Real losses are not modeled yet.",
            request.decay_per_s / std::f64::consts::PI
        ));
        Ok(Shaper {
            max_steps: ((1.0 / QUIET).ln() / request.decay_per_s / dt).ceil() as usize + 600,
            grid,
            cells,
            var_of,
            faces,
            ends,
            near,
            freqs,
            radiator,
            decay_per_s: request.decay_per_s,
            notes,
            runs: 0,
            steps_per_run: 0,
        })
    }

    pub fn variables(&self) -> usize {
        self.cells.len()
    }

    pub fn frequencies_hz(&self) -> &[f64] {
        &self.freqs
    }

    /// The objective for cell densities `density` (1 is air, 0 plastic), and its gradient
    /// with respect to each density when `gradient` is set.
    pub fn evaluate(&mut self, density: &[f64], gradient: bool) -> Evaluation {
        let at = |var: u32| -> f64 {
            if var == u32::MAX {
                1.0
            } else {
                density[var as usize]
            }
        };
        let relaxed: Vec<(u8, usize, f64)> = self
            .faces
            .iter()
            .zip(&self.ends)
            .map(|(&(axis, slot), &(low, high))| (axis, slot, at(low) * at(high)))
            .collect();
        let result = adjoint::sensitivity(&Problem {
            grid: &self.grid,
            relaxed: &relaxed,
            faces: if gradient { &self.faces } else { &[] },
            frequencies_hz: &self.freqs,
            max_steps: self.max_steps,
            quiet: QUIET,
            decay_per_s: self.decay_per_s,
        });
        self.runs += result.runs;
        self.steps_per_run = result.steps;
        let nf = self.freqs.len();
        let zero = vec![Complex64::new(0.0, 0.0); nf];
        let free = self.radiator.evaluate(&self.freqs, &zero).volume_velocity;
        let loaded = self
            .radiator
            .evaluate(&self.freqs, &result.z)
            .volume_velocity;
        let mut level = Vec::with_capacity(nf);
        let mut through = Vec::with_capacity(nf);
        for m in 0..nf {
            let h_sum: Complex64 = result.h.iter().map(|h| h[m]).sum();
            let total = loaded[m] * (Complex64::new(1.0, 0.0) - h_sum);
            through.push(Complex64::new(1.0, 0.0) - h_sum);
            level.push(20.0 * (total.norm() * self.freqs[m]).max(1e-300).log10());
        }
        let mean = level.iter().sum::<f64>() / nf as f64;
        for value in &mut level {
            *value -= mean;
        }
        let objective = level.iter().map(|x| x * x).sum::<f64>() / nf as f64;
        let mut out = Evaluation {
            objective,
            level_db: level,
            gradient: Vec::new(),
        };
        if !gradient {
            return out;
        }
        // d level / d Z_ab and d level / d H_k. The cone's volume velocity is
        // `A / (B + C*Z_ab)`; `B / C` comes from the two evaluations above.
        let to_db = 20.0 / std::f64::consts::LN_10;
        let mut c_z = Vec::with_capacity(nf);
        let mut c_h = Vec::with_capacity(nf);
        for m in 0..nf {
            let weight = 2.0 / nf as f64 * out.level_db[m] * to_db;
            let ratio = free[m] / loaded[m] - Complex64::new(1.0, 0.0);
            c_z.push(if ratio.norm() > 1e-12 {
                -weight / (result.z[m] / ratio + result.z[m])
            } else {
                Complex64::new(0.0, 0.0)
            });
            c_h.push(-weight / through[m]);
        }
        let mut d = vec![0.0; density.len()];
        for (face, &(low, high)) in self.ends.iter().enumerate() {
            let mut g = 0.0;
            for m in 0..nf {
                let index = face * nf + m;
                let mut term = c_z[m] * result.dz[index];
                for dh in &result.dh {
                    term += c_h[m] * dh[index];
                }
                g += term.re;
            }
            if low != u32::MAX {
                d[low as usize] += g * at(high);
            }
            if high != u32::MAX {
                d[high as usize] += g * at(low);
            }
        }
        out.gradient = d;
        out
    }

    /// Densities from the design variables: the smoothing filter, then the projection of
    /// sharpness `beta`.
    fn densities(&self, x: &[f64], beta: f64) -> Vec<f64> {
        let scale = (beta * 0.5).tanh() * 2.0;
        self.near
            .iter()
            .map(|list| {
                let smooth: f64 = list.iter().map(|(j, w)| w * x[*j as usize]).sum();
                ((beta * 0.5).tanh() + (beta * (smooth - 0.5)).tanh()) / scale
            })
            .collect()
    }

    /// The gradient with respect to the design variables, from the one with respect to the
    /// densities.
    fn back(&self, x: &[f64], beta: f64, d_density: &[f64]) -> Vec<f64> {
        let scale = (beta * 0.5).tanh() * 2.0;
        let mut d = vec![0.0; x.len()];
        for (list, g) in self.near.iter().zip(d_density) {
            let smooth: f64 = list.iter().map(|(j, w)| w * x[*j as usize]).sum();
            let slope = beta * (1.0 - (beta * (smooth - 0.5)).tanh().powi(2)) / scale;
            for (j, w) in list {
                d[*j as usize] += g * slope * w;
            }
        }
        d
    }

    /// Makes a plastic-or-air shape printable: plastic thinner than two cells goes, plastic
    /// that hangs in the air goes, and air sealed off from the driver is filled.
    fn printable(&self, density: &[f64]) -> Vec<bool> {
        let grid = &self.grid;
        let [nx, ny, nz] = grid.n;
        let stride = [ny * nz, nz, 1usize];
        let coords = |cell: usize| [cell / (ny * nz), (cell / nz) % ny, cell % nz];
        let var = |cell: usize| self.var_of[cell];
        let mut plastic: Vec<bool> = density.iter().map(|value| *value < 0.5).collect();
        // Solid as the print has it: walls (anything that is not air) and the plastic.
        let solid = |cell: usize, plastic: &[bool]| -> bool {
            !grid.air[cell] || (var(cell) != u32::MAX && plastic[var(cell) as usize])
        };
        // Keep plastic only where it is part of a block of 2 x 2 x 2 solid cells.
        let mut keep = vec![false; plastic.len()];
        for i in 0..nx.saturating_sub(1) {
            for j in 0..ny.saturating_sub(1) {
                for k in 0..nz.saturating_sub(1) {
                    let base = (i * ny + j) * nz + k;
                    let block: [usize; 8] = std::array::from_fn(|corner| {
                        base + (corner & 1) * stride[2]
                            + ((corner >> 1) & 1) * stride[1]
                            + ((corner >> 2) & 1) * stride[0]
                    });
                    if block.iter().all(|cell| solid(*cell, &plastic)) {
                        for cell in block {
                            if var(cell) != u32::MAX {
                                keep[var(cell) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        for (p, k) in plastic.iter_mut().zip(&keep) {
            *p = *p && *k;
        }
        let neighbors = |cell: usize| -> Vec<usize> {
            let c = coords(cell);
            let mut list = Vec::with_capacity(6);
            for axis in 0..3 {
                if c[axis] > 0 {
                    list.push(cell - stride[axis]);
                }
                if c[axis] + 1 < grid.n[axis] {
                    list.push(cell + stride[axis]);
                }
            }
            list
        };
        // Plastic must hang on a wall, directly or through other plastic.
        let mut held = vec![false; plastic.len()];
        let mut queue: Vec<usize> = Vec::new();
        for (index, &cell) in self.cells.iter().enumerate() {
            if plastic[index] && neighbors(cell).iter().any(|other| !grid.air[*other]) {
                held[index] = true;
                queue.push(cell);
            }
        }
        while let Some(cell) = queue.pop() {
            for other in neighbors(cell) {
                let v = var(other);
                if v != u32::MAX && plastic[v as usize] && !held[v as usize] {
                    held[v as usize] = true;
                    queue.push(other);
                }
            }
        }
        for (p, h) in plastic.iter_mut().zip(&held) {
            *p = *p && *h;
        }
        for _ in 0..20 {
            let mut changed = false;
            // Air the driver cannot reach is filled.
            let mut reached = vec![false; grid.air.len()];
            let mut queue: Vec<usize> = grid.source.clone();
            for &cell in &queue {
                reached[cell] = true;
            }
            while let Some(cell) = queue.pop() {
                for other in neighbors(cell) {
                    if !reached[other] && !solid(other, &plastic) {
                        reached[other] = true;
                        queue.push(other);
                    }
                }
            }
            for (index, &cell) in self.cells.iter().enumerate() {
                if !reached[cell] && !plastic[index] {
                    plastic[index] = true;
                    changed = true;
                }
            }
            // Solid that meets other solid along an edge only cannot be printed as one
            // piece: one of the two air cells at that edge is filled.
            for i in 0..nx {
                for j in 0..ny {
                    for k in 0..nz {
                        let c = [i, j, k];
                        let base = (i * ny + j) * nz + k;
                        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
                            if c[a] + 1 >= grid.n[a] || c[b] + 1 >= grid.n[b] {
                                continue;
                            }
                            let quad = [
                                base,
                                base + stride[a],
                                base + stride[b],
                                base + stride[a] + stride[b],
                            ];
                            let is: [bool; 4] = std::array::from_fn(|q| solid(quad[q], &plastic));
                            let fill = if is == [true, false, false, true] {
                                [quad[1], quad[2]]
                            } else if is == [false, true, true, false] {
                                [quad[0], quad[3]]
                            } else {
                                continue;
                            };
                            if let Some(v) =
                                fill.iter().map(|cell| var(*cell)).find(|v| *v != u32::MAX)
                            {
                                plastic[v as usize] = true;
                                changed = true;
                            }
                        }
                    }
                }
            }
            // The same for solid that meets other solid at a corner point only: a cell beside
            // one of the two is filled (the edge it then shares is dealt with next round).
            for i in 0..nx.saturating_sub(1) {
                for j in 0..ny.saturating_sub(1) {
                    for k in 0..nz.saturating_sub(1) {
                        let base = (i * ny + j) * nz + k;
                        let at = |corner: usize| {
                            base + (corner & 1) * stride[2]
                                + ((corner >> 1) & 1) * stride[1]
                                + ((corner >> 2) & 1) * stride[0]
                        };
                        for corner in 0..4usize {
                            let other = 7 - corner;
                            if !solid(at(corner), &plastic) || !solid(at(other), &plastic) {
                                continue;
                            }
                            // The six two-cell ways from one to the other through faces.
                            let joined = (0..3).any(|first| {
                                (0..3).filter(|second| *second != first).any(|second| {
                                    let one = corner ^ (1 << first);
                                    let two = one ^ (1 << second);
                                    solid(at(one), &plastic) && solid(at(two), &plastic)
                                })
                            });
                            if joined {
                                continue;
                            }
                            let beside = (0..3)
                                .flat_map(|bit| [corner ^ (1 << bit), other ^ (1 << bit)])
                                .map(|c| var(at(c)))
                                .find(|v| *v != u32::MAX && !plastic[*v as usize]);
                            if let Some(v) = beside {
                                plastic[v as usize] = true;
                                changed = true;
                            }
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        plastic
    }
}

/// Shapes the inside of `design`'s cabinet. `progress(step, steps)` is called after every
/// step and stops the work when it returns false; what was reached by then is returned.
pub fn shape_inside(
    design: &DesignFile,
    request: &ShapeRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<ShapeResult, String> {
    let mut shaper = Shaper::new(design, request, registry)?;
    let n = shaper.variables();
    let cap = request.max_plastic_fraction.clamp(0.0, 1.0) * n as f64;
    let plastic_of = |density: &[f64]| -> f64 { density.iter().map(|value| 1.0 - value).sum() };

    // Start from an even haze of as much plastic as is allowed: from all air the gradient
    // only says where a little plastic helps, and that is seldom where a block of it does.
    let start = shaper.evaluate(&vec![1.0; n], false);
    let mut x = vec![1.0 - request.max_plastic_fraction.clamp(0.0, 0.5); n];
    let mut history = vec![start.objective.sqrt()];
    let mut canceled = false;
    let stages = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];
    let per_stage = request.iterations.div_ceil(stages.len()).max(1);
    let mut step_count = 0usize;
    let density_steps = per_stage * stages.len();
    let total_steps = density_steps + request.iterations;
    let mut shaped = start.objective;
    let mut beta_now = stages[0];
    // Every candidate keeps within the plastic allowed: if it holds too much, every cell is
    // lifted toward air by the same amount.
    let capped = |shaper: &Shaper, x: &[f64], g: &[f64], scale: f64, reach: f64, beta: f64| {
        let moved = |lift: f64| -> Vec<f64> {
            x.iter()
                .zip(g)
                .map(|(value, slope)| {
                    (value - (reach * slope / scale).clamp(-reach, reach) + lift).clamp(0.0, 1.0)
                })
                .collect()
        };
        let trial = moved(0.0);
        if plastic_of(&shaper.densities(&trial, beta)) <= cap {
            return trial;
        }
        let (mut low, mut high) = (0.0, 1.0 + reach);
        for _ in 0..40 {
            let middle = 0.5 * (low + high);
            if plastic_of(&shaper.densities(&moved(middle), beta)) > cap {
                low = middle;
            } else {
                high = middle;
            }
        }
        moved(high)
    };
    'stages: for beta in stages {
        beta_now = beta;
        // The sharper projection changes the shape a little, so each stage starts afresh.
        let zero = vec![0.0; n];
        x = capped(&shaper, &x, &zero, 1.0, 0.0, beta);
        let mut current = shaper.evaluate(&shaper.densities(&x, beta), true);
        let mut reach = 0.2;
        for _ in 0..per_stage {
            let g = shaper.back(&x, beta, &current.gradient);
            // Steps are sized by a typical large slope, not the single largest, which is
            // often one cell beside the driver.
            let mut sizes: Vec<f64> = g.iter().map(|v| v.abs()).collect();
            sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let scale = sizes[(sizes.len() * 9) / 10];
            step_count += 1;
            if scale > 0.0 {
                let mut accepted = false;
                for _ in 0..4 {
                    let trial = capped(&shaper, &x, &g, scale, reach, beta);
                    let next = shaper.evaluate(&shaper.densities(&trial, beta), true);
                    if next.objective < current.objective {
                        x = trial;
                        current = next;
                        accepted = true;
                        break;
                    }
                    reach *= 0.5;
                }
                reach = if accepted {
                    (reach * 1.5).min(0.3)
                } else {
                    reach.max(0.01)
                };
            }
            history.push(current.objective.sqrt());
            shaped = current.objective;
            if !progress(step_count, total_steps) {
                canceled = true;
                break 'stages;
            }
        }
    }

    // The plastic-or-air shape, cleaned for printing, run again from scratch.
    let to_density = |plastic: &[bool]| -> Vec<f64> {
        plastic.iter().map(|p| if *p { 0.0 } else { 1.0 }).collect()
    };
    let mut plastic = shaper.printable(&shaper.densities(&x, beta_now));
    // Finishing on the plastic-or-air shape itself: the gradient there names the cells at
    // the plastic's surface whose change should help most; a batch of them is changed, the
    // shape is cleaned for printing again, and the change is kept only if a fresh run is
    // more even.
    let mut best = shaper.evaluate(&to_density(&plastic), true);
    if best.objective > start.objective {
        // Plain air is the better starting point.
        plastic = vec![false; n];
        best = shaper.evaluate(&to_density(&plastic), true);
    }
    let mut batch = (n / 40).max(1);
    let mut rounds = 0;
    while !canceled && rounds < request.iterations && batch >= 1 {
        rounds += 1;
        let count = plastic.iter().filter(|p| **p).count() as f64;
        let mut gains: Vec<(f64, usize)> = best
            .gradient
            .iter()
            .enumerate()
            .filter_map(|(index, slope)| {
                let gain = if plastic[index] { -slope } else { *slope };
                (gain > 0.0).then_some((gain, index))
            })
            .collect();
        gains.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let mut trial = plastic.clone();
        let mut added = 0.0;
        for &(_, index) in gains.iter().take(batch) {
            if trial[index] {
                trial[index] = false;
                added -= 1.0;
            } else if count + added + 1.0 <= cap {
                trial[index] = true;
                added += 1.0;
            }
        }
        let trial = shaper.printable(&to_density(&trial));
        let next = if trial == plastic {
            None
        } else {
            Some(shaper.evaluate(&to_density(&trial), true))
        };
        match next {
            Some(next) if next.objective < best.objective => {
                plastic = trial;
                best = next;
                history.push(best.objective.sqrt());
            }
            _ => batch /= 2,
        }
        if !progress(density_steps + rounds, total_steps) {
            canceled = true;
        }
    }
    let hard = to_density(&plastic);
    let after = shaper.evaluate(&hard, false);

    let grid = &shaper.grid;
    let [_, ny, nz] = grid.n;
    let stride = [ny * nz, nz, 1usize];
    let coords = |cell: usize| [cell / (ny * nz), (cell / nz) % ny, cell % nz];
    // The plastic's cells, and the wall cells they touch: the grid's cells need not end
    // exactly at a wall, and the element counts only what lies in the air, so the extra
    // layer closes any gap between the plastic and the wall it hangs on.
    let mut marked: Vec<usize> = Vec::new();
    for (index, &cell) in shaper.cells.iter().enumerate() {
        if !plastic[index] {
            continue;
        }
        marked.push(cell);
        let c = coords(cell);
        for axis in 0..3 {
            if c[axis] > 0 && !grid.air[cell - stride[axis]] {
                marked.push(cell - stride[axis]);
            }
            if c[axis] + 1 < grid.n[axis] && !grid.air[cell + stride[axis]] {
                marked.push(cell + stride[axis]);
            }
        }
    }
    let mut low = [0usize; 3];
    let mut high = [1usize; 3];
    for (count, &cell) in marked.iter().enumerate() {
        let c = coords(cell);
        for axis in 0..3 {
            low[axis] = if count == 0 {
                c[axis]
            } else {
                low[axis].min(c[axis])
            };
            high[axis] = if count == 0 {
                c[axis] + 1
            } else {
                high[axis].max(c[axis] + 1)
            };
        }
    }
    let size: [usize; 3] = std::array::from_fn(|axis| high[axis] - low[axis]);
    let mut solid = vec![false; size[0] * size[1] * size[2]];
    for &cell in &marked {
        let c = coords(cell);
        solid[((c[0] - low[0]) * size[1] + (c[1] - low[1])) * size[2] + (c[2] - low[2])] = true;
    }
    let cell_m3 = grid.dx.powi(3);
    Ok(ShapeResult {
        dx_m: grid.dx,
        min_m: std::array::from_fn(|axis| grid.origin_m[axis] + low[axis] as f64 * grid.dx),
        size,
        solid,
        plastic_m3: plastic.iter().filter(|p| **p).count() as f64 * cell_m3,
        region_m3: n as f64 * cell_m3,
        frequencies_hz: shaper.freqs.clone(),
        level_before_db: start.level_db,
        level_after_db: after.level_db,
        uneven_before_db: start.objective.sqrt(),
        uneven_shaped_db: shaped.sqrt(),
        uneven_after_db: after.objective.sqrt(),
        history_db: history,
        runs: shaper.runs,
        steps_per_run: shaper.steps_per_run,
        canceled,
        notes: shaper.notes.clone(),
    })
}

/// The id of the element that shaping writes; shaping again replaces it.
pub const SHAPED_ID: &str = "shaped";

/// `design` with the shaped plastic as a `voxels` solid (README section 6.2).
pub fn apply(design: &DesignFile, result: &ShapeResult) -> DesignFile {
    let mut out = design.clone();
    let solids = &mut out.speaker.enclosure.solids;
    solids.retain(|solid| solid.get("id").and_then(serde_json::Value::as_str) != Some(SHAPED_ID));
    if result.solid.iter().any(|cell| *cell) {
        solids.push(crate::solids::element(
            SHAPED_ID,
            result.min_m,
            result.dx_m,
            result.size,
            &result.solid,
        ));
    }
    out
}
