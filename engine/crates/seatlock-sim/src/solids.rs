//! Plastic placed inside the cabinet's air, cell by cell: the internal element `voxels`
//! (README section 6.2). It is what "Shape the inside" writes into a design.
//!
//! ```text
//! { "id": "shaped", "kind": "voxels", "min_m": [x, y, z], "dx_m": 0.01, "size": [nx, ny, nz],
//!   "cells": "12 3 40 5" }
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

/// The design's `voxels` solids, and the ids and kinds of solids of any other kind, which
/// this build leaves out.
/// The id and the kind of a solid this build does not make.
pub type Unknown = (String, String);

pub fn solids(design: &DesignFile) -> Result<(Vec<Voxels>, Vec<Unknown>), String> {
    let mut out = Vec::new();
    let mut unknown = Vec::new();
    for (index, raw) in design.speaker.enclosure.solids.iter().enumerate() {
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}", index + 1));
        let kind = raw.get("kind").and_then(Value::as_str).unwrap_or("");
        if kind != "voxels" {
            unknown.push((id, kind.to_string()));
            continue;
        }
        let triple = |name: &str| -> Option<Vec<f64>> {
            let list: Vec<f64> = raw
                .get(name)?
                .as_array()?
                .iter()
                .filter_map(Value::as_f64)
                .collect();
            (list.len() == 3).then_some(list)
        };
        let min = triple("min_m").ok_or(format!("Solid {id} has no min_m."))?;
        let size = triple("size").ok_or(format!("Solid {id} has no size."))?;
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
        out.push(Voxels {
            id,
            min_m: [min[0], min[1], min[2]],
            dx_m,
            size,
            solid,
        });
    }
    Ok((out, unknown))
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
