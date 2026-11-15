//! Plastic placed inside the cabinet's air: `voxels` (README section 6.2, what "Shape the
//! inside" writes) and `brace` bars that take air, print, and split the walls they join.
//!
//! ```text
//! { "id": "shaped", "kind": "voxels", "min_m": [x, y, z], "dx_m": 0.01, "size": [nx, ny, nz],
//!   "cells": "12 3 40 5" }
//! { "id": "brace-1", "kind": "brace", "min_m": [0.008, 0.080, 0.120], "max_m": [0.152, 0.100, 0.130] }
//! ```
//!
//! `cells` is run-length encoded in the order `(i * ny + j) * nz + k`: so many cells of air,
//! so many of plastic, and so on, starting with air. Only the part of a cell that lies in the
//! cabinet's air counts: a cell that reaches into a wall adds nothing there.

use seatlock_core::design::DesignFile;
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct Voxels {
    pub id: String,
    pub min_m: [f64; 3],
    pub dx_m: f64,
    pub size: [usize; 3],
    pub solid: Vec<bool>,
}

/// A rectangular bar of plastic inside the air (README 6.2). It takes volume, prints, and
/// fills the 3D grid like `voxels`; walls it reaches split into smaller plates (README 5.11).
#[derive(Clone, Debug)]
pub struct Brace {
    pub id: String,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
}

/// The id and the kind of a solid this build does not make.
pub type Unknown = (String, String);

/// Every solid the design places in its air.
#[derive(Clone, Debug, Default)]
pub struct Placed {
    pub voxels: Vec<Voxels>,
    pub braces: Vec<Brace>,
    pub unknown: Vec<Unknown>,
}

impl Placed {
    pub fn plastic_m3(&self, in_air: &dyn Fn([f64; 3]) -> bool) -> f64 {
        self.voxels
            .iter()
            .map(|solid| solid.plastic_m3(in_air))
            .sum::<f64>()
            + self
                .braces
                .iter()
                .map(|brace| brace.plastic_m3(in_air))
                .sum::<f64>()
    }

    pub fn contains(&self, p: [f64; 3]) -> bool {
        self.voxels.iter().any(|solid| solid.contains(p))
            || self.braces.iter().any(|brace| brace.contains(p))
    }

    /// Boxes to merge into the print tree: voxel merges, then each brace as one box.
    pub fn print_boxes(&self) -> Vec<([f64; 3], [f64; 3])> {
        let mut out: Vec<_> = self.voxels.iter().flat_map(Voxels::boxes).collect();
        out.extend(self.braces.iter().map(|b| (b.min_m, b.max_m)));
        out
    }
}

pub fn encode(solid: &[bool]) -> String {
    let mut runs = Vec::new();
    let mut state = false;
    let mut count = 0usize;
    for &cell in solid {
        if cell != state {
            runs.push(count.to_string());
            state = cell;
            count = 0;
        }
        count += 1;
    }
    runs.push(count.to_string());
    runs.join(" ")
}

pub fn decode(text: &str, cells: usize) -> Result<Vec<bool>, String> {
    let mut out = Vec::with_capacity(cells);
    let mut state = false;
    for run in text.split_whitespace() {
        let count: usize = run.parse().map_err(|_| "its cells are not numbers.")?;
        if out.len() + count > cells {
            return Err("its cells are more than its size holds.".into());
        }
        out.extend(std::iter::repeat_n(state, count));
        state = !state;
    }
    if out.len() != cells {
        return Err("its cells are fewer than its size holds.".into());
    }
    Ok(out)
}

/// The element for a design file.
pub fn element(id: &str, min_m: [f64; 3], dx_m: f64, size: [usize; 3], solid: &[bool]) -> Value {
    json!({ "id": id, "kind": "voxels", "min_m": min_m, "dx_m": dx_m, "size": size,
            "cells": encode(solid) })
}

fn triple(raw: &Value, name: &str) -> Option<[f64; 3]> {
    let list: Vec<f64> = raw
        .get(name)?
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    (list.len() == 3).then_some([list[0], list[1], list[2]])
}

/// The design's placed solids: voxels, braces, and any other kind left out with a warning.
pub fn solids(design: &DesignFile) -> Result<Placed, String> {
    let mut out = Placed::default();
    for (index, raw) in design.speaker.enclosure.solids.iter().enumerate() {
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}", index + 1));
        let kind = raw.get("kind").and_then(Value::as_str).unwrap_or("");
        match kind {
            "voxels" => {
                let min = triple(raw, "min_m").ok_or(format!("Solid {id} has no min_m."))?;
                let size = triple(raw, "size").ok_or(format!("Solid {id} has no size."))?;
                let dx_m = raw
                    .get("dx_m")
                    .and_then(Value::as_f64)
                    .filter(|dx| *dx > 0.0)
                    .ok_or(format!("Solid {id} has no dx_m."))?;
                let size = [size[0] as usize, size[1] as usize, size[2] as usize];
                let cells = size[0] * size[1] * size[2];
                if cells == 0 || cells > 50_000_000 {
                    return Err(format!("Solid {id} has no usable size."));
                }
                let solid = decode(
                    raw.get("cells").and_then(Value::as_str).unwrap_or(""),
                    cells,
                )
                .map_err(|problem| format!("Solid {id}: {problem}"))?;
                out.voxels.push(Voxels {
                    id,
                    min_m: min,
                    dx_m,
                    size,
                    solid,
                });
            }
            "brace" => {
                let min = triple(raw, "min_m").ok_or(format!("Solid {id} has no min_m."))?;
                let max = triple(raw, "max_m").ok_or(format!("Solid {id} has no max_m."))?;
                if (0..3).any(|i| max[i] <= min[i]) {
                    return Err(format!("Solid {id} has no size."));
                }
                out.braces.push(Brace {
                    id,
                    min_m: min,
                    max_m: max,
                });
            }
            _ => out.unknown.push((id, kind.to_string())),
        }
    }
    Ok(out)
}

impl Brace {
    pub fn contains(&self, p: [f64; 3]) -> bool {
        (0..3).all(|i| p[i] > self.min_m[i] && p[i] < self.max_m[i])
    }

    pub fn size_m(&self) -> [f64; 3] {
        std::array::from_fn(|i| self.max_m[i] - self.min_m[i])
    }

    /// The plastic that lies where `in_air` holds, from 8 x 8 x 8 samples.
    pub fn plastic_m3(&self, in_air: &dyn Fn([f64; 3]) -> bool) -> f64 {
        const N: usize = 8;
        let size = self.size_m();
        let mut hits = 0usize;
        for a in 0..N {
            for b in 0..N {
                for d in 0..N {
                    let sub = [a, b, d];
                    let p: [f64; 3] = std::array::from_fn(|axis| {
                        self.min_m[axis] + (sub[axis] as f64 + 0.5) / N as f64 * size[axis]
                    });
                    if in_air(p) {
                        hits += 1;
                    }
                }
            }
        }
        hits as f64 * size[0] * size[1] * size[2] / (N * N * N) as f64
    }
}

impl Voxels {
    pub fn contains(&self, p: [f64; 3]) -> bool {
        let mut at = [0usize; 3];
        for axis in 0..3 {
            let step = ((p[axis] - self.min_m[axis]) / self.dx_m).floor();
            if step < 0.0 || step >= self.size[axis] as f64 {
                return false;
            }
            at[axis] = step as usize;
        }
        self.solid[(at[0] * self.size[1] + at[1]) * self.size[2] + at[2]]
    }

    /// The plastic that lies where `in_air` holds, in m^3, from 6 x 6 x 6 points per cell.
    pub fn plastic_m3(&self, in_air: &dyn Fn([f64; 3]) -> bool) -> f64 {
        const N: usize = 6;
        let [_, ny, nz] = self.size;
        let mut hits = 0usize;
        for (cell, _) in self.solid.iter().enumerate().filter(|(_, s)| **s) {
            let c = [cell / (ny * nz), (cell / nz) % ny, cell % nz];
            for a in 0..N {
                for b in 0..N {
                    for d in 0..N {
                        let sub = [a, b, d];
                        let p: [f64; 3] = std::array::from_fn(|axis| {
                            self.min_m[axis]
                                + (c[axis] as f64 + (sub[axis] as f64 + 0.5) / N as f64) * self.dx_m
                        });
                        if in_air(p) {
                            hits += 1;
                        }
                    }
                }
            }
        }
        hits as f64 * self.dx_m.powi(3) / (N * N * N) as f64
    }

    /// The plastic as boxes that do not overlap, cells merged along z, then y, then x.
    pub fn boxes(&self) -> Vec<([f64; 3], [f64; 3])> {
        let [nx, ny, nz] = self.size;
        let at = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
        let mut left = self.solid.clone();
        let mut out = Vec::new();
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    if !left[at(i, j, k)] {
                        continue;
                    }
                    let mut k1 = k + 1;
                    while k1 < nz && left[at(i, j, k1)] {
                        k1 += 1;
                    }
                    let mut j1 = j + 1;
                    while j1 < ny && (k..k1).all(|kk| left[at(i, j1, kk)]) {
                        j1 += 1;
                    }
                    let mut i1 = i + 1;
                    while i1 < nx && (j..j1).all(|jj| (k..k1).all(|kk| left[at(i1, jj, kk)])) {
                        i1 += 1;
                    }
                    for ii in i..i1 {
                        for jj in j..j1 {
                            for kk in k..k1 {
                                left[at(ii, jj, kk)] = false;
                            }
                        }
                    }
                    let corner = |c: [usize; 3]| -> [f64; 3] {
                        std::array::from_fn(|axis| self.min_m[axis] + c[axis] as f64 * self.dx_m)
                    };
                    out.push((corner([i, j, k]), corner([i1, j1, k1])));
                }
            }
        }
        out
    }
}
