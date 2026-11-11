//! Print splitting (README section 6.4): cut the printed cabinet into parts that fit the
//! printer, and drill alignment pin holes across every cut.
//!
//! The engine chooses the cuts and the pins; the page cuts the solid with manifold-3d and
//! exports one 3MF object per part and per pin. Cuts are evenly spaced along each axis, and
//! the cabinet may sit on the bed any way round: the fewest parts wins.

use serde::Serialize;
use serde_json::{json, Value};

/// Pin holes are 4.2 mm across for a printed 4 mm pin, 8 mm deep on each side of the cut,
/// with at least 1.2 mm of plastic around them. Lab defaults (STATUS decisions).
pub const PIN_HOLE_RADIUS_M: f64 = 0.0021;
pub const PIN_RADIUS_M: f64 = 0.002;
pub const PIN_HOLE_DEPTH_M: f64 = 0.008;
pub const PIN_LENGTH_M: f64 = 0.015;
const PIN_MARGIN_M: f64 = 0.0012;
const PIN_SPACING_M: f64 = 0.02;
const PINS_PER_JOINT: usize = 3;

#[derive(Clone, Debug, Serialize)]
pub struct SplitPart {
    pub name: String,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct Pin {
    pub center_m: [f64; 3],
    /// The axis the pin crosses the cut along: 0 for x, 1 for y, 2 for z.
    pub axis: usize,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct PrintSplit {
    /// The cells the solid is cut into; empty when it fits the printer whole.
    pub parts: Vec<SplitPart>,
    pub pins: Vec<Pin>,
    pub pin_radius_m: f64,
    pub pin_length_m: f64,
    /// Plastic the pin holes take out of the cabinet.
    pub pin_holes_m3: f64,
    /// Plastic of the printed pins themselves.
    pub pins_m3: f64,
    pub notes: Vec<String>,
}

/// How many pieces each axis is cut into, for the best way to stand the cabinet on the bed.
pub fn pieces(size: [f64; 3], bed: [f64; 3]) -> [usize; 3] {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut best = [usize::MAX; 3];
    let mut best_count = usize::MAX;
    for order in orders {
        let n: Vec<usize> = (0..3)
            .map(|axis| (size[axis] / bed[order[axis]] - 1e-9).ceil().max(1.0) as usize)
            .collect();
        let count = n[0] * n[1] * n[2];
        if count < best_count {
            best_count = count;
            best = [n[0], n[1], n[2]];
        }
    }
    best
}

/// Plans the split of a cabinet `size` big for a bed of `bed`. `solid_at` says where there
/// is plastic to hold a pin hole.
pub fn plan(size: [f64; 3], bed: [f64; 3], solid_at: &dyn Fn([f64; 3]) -> bool) -> PrintSplit {
    let n = pieces(size, bed);
    let mut split = PrintSplit {
        pin_radius_m: PIN_RADIUS_M,
        pin_length_m: PIN_LENGTH_M,
        ..PrintSplit::default()
    };
    if n == [1, 1, 1] {
        return split;
    }
    let cut = |axis: usize, k: usize| size[axis] * k as f64 / n[axis] as f64;
    // Cells reach a little past the outside so no sliver is lost to rounding.
    let reach = |axis: usize, k: usize, high: bool| {
        let edge = cut(axis, k);
        if (!high && k == 0) || (high && k == n[axis]) {
            edge + if high { 0.001 } else { -0.001 }
        } else {
            edge
        }
    };
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                split.parts.push(SplitPart {
                    name: format!("part {}", split.parts.len() + 1),
                    min_m: [reach(0, i, false), reach(1, j, false), reach(2, k, false)],
                    max_m: [
                        reach(0, i + 1, true),
                        reach(1, j + 1, true),
                        reach(2, k + 1, true),
                    ],
                });
            }
        }
    }
    // Pins: across each cut, for each pair of cells that meet there.
    let step = 0.002;
    for axis in 0..3 {
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        for plane in 1..n[axis] {
            let at = cut(axis, plane);
            for ia in 0..n[a] {
                for ib in 0..n[b] {
                    let (a0, a1) = (cut(a, ia), cut(a, ia + 1));
                    let (b0, b1) = (cut(b, ib), cut(b, ib + 1));
                    let mut candidates = Vec::new();
                    let mut u = a0 + step / 2.0;
                    while u < a1 {
                        let mut v = b0 + step / 2.0;
                        while v < b1 {
                            let mut p = [0.0; 3];
                            p[axis] = at;
                            p[a] = u;
                            p[b] = v;
                            if pin_fits(p, axis, a, b, solid_at) {
                                candidates.push(p);
                            }
                            v += step;
                        }
                        u += step;
                    }
                    let chosen = spread(&candidates, PINS_PER_JOINT, PIN_SPACING_M);
                    if chosen.len() < 2 {
                        split.notes.push(format!(
                            "The cut at {:.0} mm has no room for two alignment pins between \
                             some parts; glue those parts and line them up by hand.",
                            at * 1000.0
                        ));
                    }
                    for center in chosen {
                        split.pins.push(Pin {
                            center_m: center,
                            axis,
                        });
                    }
                }
            }
        }
    }
    let count = split.pins.len() as f64;
    let pi = std::f64::consts::PI;
    split.pin_holes_m3 = count * pi * PIN_HOLE_RADIUS_M.powi(2) * 2.0 * PIN_HOLE_DEPTH_M;
    split.pins_m3 = count * pi * PIN_RADIUS_M.powi(2) * PIN_LENGTH_M;
    split
}

/// A hole fits at `p` when there is plastic all around it, the whole depth on both sides.
fn pin_fits(
    p: [f64; 3],
    axis: usize,
    a: usize,
    b: usize,
    solid_at: &dyn Fn([f64; 3]) -> bool,
) -> bool {
    let ring = PIN_HOLE_RADIUS_M + PIN_MARGIN_M;
    let deep = PIN_HOLE_DEPTH_M + PIN_MARGIN_M;
    for along in [-deep, 0.0, deep] {
        for (da, db) in [
            (0.0, 0.0),
            (ring, 0.0),
            (-ring, 0.0),
            (0.0, ring),
            (0.0, -ring),
            (0.7 * ring, 0.7 * ring),
            (-0.7 * ring, 0.7 * ring),
            (0.7 * ring, -0.7 * ring),
            (-0.7 * ring, -0.7 * ring),
        ] {
            let mut q = p;
            q[axis] += along;
            q[a] += da;
            q[b] += db;
            if !solid_at(q) {
                return false;
            }
        }
    }
    true
}

/// Up to `count` points from `candidates`, as far apart as possible and at least `spacing`
/// apart: the first is the farthest from their middle, each next the farthest from those
/// already chosen. Deterministic.
fn spread(candidates: &[[f64; 3]], count: usize, spacing: f64) -> Vec<[f64; 3]> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let distance = |p: [f64; 3], q: [f64; 3]| {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    let mut middle = [0.0; 3];
    for p in candidates {
        for i in 0..3 {
            middle[i] += p[i] / candidates.len() as f64;
        }
    }
    let first = candidates
        .iter()
        .copied()
        .max_by(|p, q| {
            distance(*p, middle)
                .partial_cmp(&distance(*q, middle))
                .unwrap()
        })
        .unwrap();
    let mut chosen = vec![first];
    while chosen.len() < count {
        let next = candidates
            .iter()
            .copied()
            .map(|p| {
                let nearest = chosen
                    .iter()
                    .map(|c| distance(p, *c))
                    .fold(f64::INFINITY, f64::min);
                (p, nearest)
            })
            .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        match next {
            Some((p, nearest)) if nearest >= spacing => chosen.push(p),
            _ => break,
        }
    }
    chosen
}

/// The pin holes as CSG cylinders, to subtract from the cabinet.
pub fn hole_tree(split: &PrintSplit) -> Vec<Value> {
    split
        .pins
        .iter()
        .map(|pin| {
            let mut base = pin.center_m;
            base[pin.axis] -= PIN_HOLE_DEPTH_M;
            let mut axis = [0.0; 3];
            axis[pin.axis] = 1.0;
            json!({ "op": "cylinder", "base_m": base, "axis": axis,
                    "length_m": 2.0 * PIN_HOLE_DEPTH_M, "radius_m": PIN_HOLE_RADIUS_M,
                    "segments": 96 })
        })
        .collect()
}
