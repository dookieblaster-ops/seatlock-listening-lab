//! An imported cabinet (README 6.2, outer shape `mesh`): an assembly of closed triangle
//! meshes, each part solid or damping, with mounting points the user placed on its surface.
//!
//! ```jsonc
//! { "kind": "mesh",
//!   "parts": [ { "name": "shell", "role": "solid", "material": "pla",
//!                "vertices_m": "<base64 of little-endian f32 x, y, z>",
//!                "triangles": "<base64 of little-endian u32 a, b, c>" } ],
//!   "points": [ { "id": "p1", "point_m": [0.08, 0.0, 0.15], "normal": [0.0, -1.0, 0.0] } ] }
//! ```
//!
//! Space is sorted on a grid of columns along `z`: every column's crossings with each part
//! give that part's inside exactly along the column. Air that cannot be reached from outside
//! the assembly without passing through a solid part is the cabinet's air; a damping part
//! counts as air the sound passes through. Volumes integrate the exact crossings over fine
//! columns, so only the sampling across the columns is approximate.

use seatlock_core::registry::{AirRegions, MouldedOpening, MountPose, OuterShape, Registry};
use serde_json::{json, Map, Value};

use crate::{csg_difference, csg_hole, csg_union};

/// What a place in space is.
const AIR: u8 = 0;
const DAMPING: u8 = 1;
const SOLID: u8 = 2;
const OUTSIDE: u8 = 3;
/// Held closed while the outside is flooded in: a marked hole's footprint.
const BLOCK: u8 = 4;

pub struct Part {
    pub name: String,
    pub damping: bool,
    pub material: String,
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub volume_m3: f64,
    /// Flow resistivity in Pa*s/m^2 for a damping part, when the design gives it.
    pub resistivity: Option<f64>,
}

/// What a mounting point marks in the model itself (README 6.2, `mesh`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hole {
    /// A hole in the model that what is mounted here fills (a driver's cut-out and recess).
    Seal,
    /// The mouth of a duct moulded into the model, open to the outside air.
    Opening,
}

struct Point {
    id: String,
    position: [f64; 3],
    normal: [f64; 3],
    t1: [f64; 3],
    t2: [f64; 3],
    hole: Option<Hole>,
}

/// A marked hole as found on the coarse grid.
struct FoundHole {
    point: usize,
    /// The grid axis the hole runs along, and the step inward along it.
    axis: usize,
    inward: isize,
    /// Cells of the first layer under the surface that are open.
    mouth_cells: Vec<usize>,
    /// How many layers the hole runs through the wall.
    layers: usize,
}

/// Column crossings: for each column, each part's sorted crossing heights.
struct Columns {
    /// Per column, per part: inside intervals `[z0, z1]`.
    inside: Vec<Vec<(u16, f64, f64)>>,
    open_parts: Vec<bool>,
}

pub struct MeshShape {
    parts: Vec<Part>,
    points: Vec<Point>,
    min: [f64; 3],
    max: [f64; 3],
    /// The coarse grid's cells, `(i * ny + j) * nz + k`.
    cells: Vec<u8>,
    grid_origin: [f64; 3],
    grid_dx: f64,
    grid_n: [usize; 3],
    air_m3: f64,
    damping_m3: f64,
    notes: Vec<String>,
    /// Enclosed air spaces, when the model marks holes; `None` otherwise.
    regions: Option<std::sync::Arc<AirRegions>>,
    /// Per point: the space behind it, and the wall it sits in measured through its hole.
    behind: Vec<Option<usize>>,
    hole_depth_m: Vec<Option<f64>>,
    openings: Vec<MouldedOpening>,
}

pub fn register(registry: &mut Registry) {
    registry.register_outer("mesh", parse);
}

fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a') as u32 + 26,
            b'0'..=b'9' => (c - b'0') as u32 + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = value(c).ok_or("the mesh data is not base64.")?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

pub fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let v = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            if i <= n {
                out.push(TABLE[((v >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(a, a).sqrt();
    (n > 1e-12).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

fn parse_part(raw: &Value, index: usize) -> Result<Part, String> {
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("part {}", index + 1));
    let floats = decode_base64(
        raw.get("vertices_m")
            .and_then(Value::as_str)
            .ok_or(format!("{name} has no vertices."))?,
    )?;
    let ints = decode_base64(
        raw.get("triangles")
            .and_then(Value::as_str)
            .ok_or(format!("{name} has no triangles."))?,
    )?;
    let vertices: Vec<[f64; 3]> = floats
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]) as f64;
            [f(0), f(4), f(8)]
        })
        .collect();
    let triangles: Vec<[u32; 3]> = ints
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| {
            let u = |i: usize| u32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
            [u(0), u(4), u(8)]
        })
        .collect();
    if triangles.is_empty() {
        return Err(format!("{name} has no triangles."));
    }
    if triangles
        .iter()
        .flatten()
        .any(|&i| i as usize >= vertices.len())
    {
        return Err(format!("{name} names a vertex it does not have."));
    }
    // The signed volume of a closed mesh (divergence theorem); its sign says the winding.
    let mut volume = 0.0;
    for t in &triangles {
        let [a, b, c] = t.map(|i| vertices[i as usize]);
        volume += dot(a, cross(b, c)) / 6.0;
    }
    let role = raw.get("role").and_then(Value::as_str).unwrap_or("solid");
    Ok(Part {
        name,
        damping: role == "damping",
        material: raw
            .get("material")
            .and_then(Value::as_str)
            .unwrap_or(if role == "damping" { "" } else { "pla" })
            .to_string(),
        vertices,
        triangles,
        volume_m3: volume.abs(),
        resistivity: raw
            .get("flow_resistivity_pa_s_m2")
            .and_then(Value::as_f64)
            .or_else(|| {
                // The damping material's own record (README 9.1, `catalog/damping/`).
                let id = raw.get("material").and_then(Value::as_str)?;
                seatlock_core::catalog::all("catalog/damping/")
                    .into_iter()
                    .find(|r| r.get("id").and_then(Value::as_str) == Some(id))?
                    .get("flow_resistivity_pa_s_m2")?
                    .as_f64()
            }),
    })
}

/// Every part's inside along the columns of a grid over `[x0, x0 + nx*dx]` by the same in y.
fn columns(parts: &[Part], origin: [f64; 2], dx: f64, nx: usize, ny: usize) -> Columns {
    let mut hits: Vec<Vec<(u16, f64)>> = vec![Vec::new(); nx * ny];
    for (p, part) in parts.iter().enumerate() {
        for t in &part.triangles {
            let [a, b, c] = t.map(|i| part.vertices[i as usize]);
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            if area.abs() < 1e-18 {
                continue;
            }
            let lo = |k: usize| a[k].min(b[k]).min(c[k]);
            let hi = |k: usize| a[k].max(b[k]).max(c[k]);
            let i0 = (((lo(0) - origin[0]) / dx - 0.5).ceil().max(0.0)) as usize;
            let i1 = (((hi(0) - origin[0]) / dx - 0.5).floor()).min(nx as f64 - 1.0);
            let j0 = (((lo(1) - origin[1]) / dx - 0.5).ceil().max(0.0)) as usize;
            let j1 = (((hi(1) - origin[1]) / dx - 0.5).floor()).min(ny as f64 - 1.0);
            if i1 < 0.0 || j1 < 0.0 {
                continue;
            }
            for i in i0..=i1 as usize {
                let x = origin[0] + (i as f64 + 0.5) * dx;
                for j in j0..=j1 as usize {
                    let y = origin[1] + (j as f64 + 0.5) * dx;
                    // Barycentric weights of (x, y) in the projected triangle.
                    let w1 = ((x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (y - a[1])) / area;
                    let w2 = ((b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1])) / area;
                    let w0 = 1.0 - w1 - w2;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                    hits[i * ny + j].push((p as u16, z));
                }
            }
        }
    }
    let mut open_parts = vec![false; parts.len()];
    let inside = hits
        .into_iter()
        .map(|mut list| {
            list.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
            let mut out = Vec::new();
            let mut k = 0;
            while k < list.len() {
                let part = list[k].0;
                let mut end = k;
                while end < list.len() && list[end].0 == part {
                    end += 1;
                }
                let zs: Vec<f64> = list[k..end].iter().map(|h| h.1).collect();
                if zs.len() % 2 == 1 {
                    open_parts[part as usize] = true;
                }
                for pair in zs.as_chunks::<2>().0 {
                    out.push((part, pair[0], pair[1]));
                }
                k = end;
            }
            out
        })
        .collect();
    Columns { inside, open_parts }
}

impl Columns {
    /// What the parts make of height `z` in column `c`: solid wins over damping.
    fn state(&self, parts: &[Part], c: usize, z: f64) -> u8 {
        let mut state = AIR;
        for &(p, z0, z1) in &self.inside[c] {
            if z >= z0 && z <= z1 {
                if parts[p as usize].damping {
                    state = state.max(DAMPING);
                } else {
                    return SOLID;
                }
            }
        }
        state
    }
}

pub fn parse(fields: &Map<String, Value>) -> Result<Box<dyn OuterShape>, String> {
    Ok(Box::new(MeshShape::new(fields)?))
}

impl MeshShape {
    pub fn new(fields: &Map<String, Value>) -> Result<Self, String> {
        let raw_parts = fields
            .get("parts")
            .and_then(Value::as_array)
            .ok_or("an imported cabinet needs its parts.")?;
        let parts: Vec<Part> = raw_parts
            .iter()
            .enumerate()
            .map(|(i, raw)| parse_part(raw, i))
            .collect::<Result<_, _>>()?;
        if !parts.iter().any(|p| !p.damping) {
            return Err("an imported cabinet needs at least one solid part.".into());
        }
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for v in parts.iter().flat_map(|p| &p.vertices) {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
        let size = sub(max, min);
        if size.iter().any(|s| *s <= 0.0 || !s.is_finite()) {
            return Err("the imported parts have no size.".into());
        }
        // A coarse grid of about four million cells at most, no finer than 2 mm, padded by
        // two cells so the outside is connected all round.
        let box_volume = size[0] * size[1] * size[2];
        let dx = (box_volume / 4.0e6).cbrt().max(0.002);
        // A small offset keeps sample columns off axis-aligned edges and faces.
        let shift = 0.0137 * dx;
        let grid_origin = [
            min[0] - 2.0 * dx + shift,
            min[1] - 2.0 * dx + shift,
            min[2] - 2.0 * dx + shift,
        ];
        let n: [usize; 3] = std::array::from_fn(|k| (size[k] / dx).ceil() as usize + 4);
        // Fine columns, four per cell each way: the cells' states are voted from them, and
        // the volumes integrated along them.
        let fine_dx = dx / 4.0;
        let fine_n = [n[0] * 4, n[1] * 4];
        let fine = columns(
            &parts,
            [grid_origin[0], grid_origin[1]],
            fine_dx,
            fine_n[0],
            fine_n[1],
        );
        // A cell is solid when at least half its samples are (16 columns by 2 heights), so a
        // glued seam between two printed parts, thinner than half a cell, stays closed, and a
        // wall at least half a cell thick is never lost between samples.
        let mut cells = vec![AIR; n[0] * n[1] * n[2]];
        for i in 0..n[0] {
            for j in 0..n[1] {
                let c = i * n[1] + j;
                for k in 0..n[2] {
                    let z = grid_origin[2] + (k as f64 + 0.5) * dx;
                    let (mut solid, mut damping) = (0, 0);
                    for a in 0..4 {
                        for b in 0..4 {
                            let fc = (4 * i + a) * fine_n[1] + 4 * j + b;
                            for dz in [-0.25 * dx, 0.25 * dx] {
                                match fine.state(&parts, fc, z + dz) {
                                    SOLID => solid += 1,
                                    DAMPING => damping += 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                    cells[c * n[2] + k] = if solid >= 16 {
                        SOLID
                    } else if damping >= 16 {
                        DAMPING
                    } else {
                        AIR
                    };
                }
            }
        }
        let points: Vec<Point> = fields
            .get("points")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|raw| {
                        let id = raw.get("id")?.as_str()?.to_string();
                        let arr = |key: &str| -> Option<[f64; 3]> {
                            let a = raw.get(key)?.as_array()?;
                            Some([
                                a.first()?.as_f64()?,
                                a.get(1)?.as_f64()?,
                                a.get(2)?.as_f64()?,
                            ])
                        };
                        let position = arr("point_m")?;
                        let normal = unit(arr("normal")?)?;
                        // Up the surface is world up where the surface stands; otherwise
                        // toward the back.
                        let up = if normal[2].abs() < 0.9 {
                            [0.0, 0.0, 1.0]
                        } else {
                            [0.0, 1.0, 0.0]
                        };
                        let t2 = unit(sub(up, {
                            let d = dot(up, normal);
                            [normal[0] * d, normal[1] * d, normal[2] * d]
                        }))?;
                        let t1 = cross(t2, normal);
                        let hole = match raw.get("hole").and_then(Value::as_str) {
                            Some("seal") => Some(Hole::Seal),
                            Some("opening") => Some(Hole::Opening),
                            _ => None,
                        };
                        Some(Point {
                            id,
                            position,
                            normal,
                            t1,
                            t2,
                            hole,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Marked holes: each held closed while the outside floods in (README 6.2, `mesh`).
        let mut found: Vec<FoundHole> = Vec::new();
        let mut held: Vec<(usize, u8, bool)> = Vec::new();
        for (index, point) in points.iter().enumerate() {
            let Some(hole) = point.hole else { continue };
            let hole = find_hole(&cells, n, grid_origin, dx, index, point, hole)
                .map_err(|e| format!("mounting point {}: {e}", point.id))?;
            let seal = points[index].hole == Some(Hole::Seal);
            for &cell in &hole.blocked {
                if cells[cell] != BLOCK {
                    held.push((cell, cells[cell], seal));
                    cells[cell] = BLOCK;
                }
            }
            found.push(hole.found);
        }
        // Flood the outside in from the corner, through everything that is not solid.
        let index = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
        let mut stack = vec![(0usize, 0usize, 0usize)];
        cells[0] = OUTSIDE;
        while let Some((i, j, k)) = stack.pop() {
            let mut visit = |a: usize, b: usize, c: usize, stack: &mut Vec<_>| {
                let at = index(a, b, c);
                if cells[at] != SOLID && cells[at] != OUTSIDE && cells[at] != BLOCK {
                    cells[at] = OUTSIDE;
                    stack.push((a, b, c));
                }
            };
            if i > 0 {
                visit(i - 1, j, k, &mut stack);
            }
            if i + 1 < n[0] {
                visit(i + 1, j, k, &mut stack);
            }
            if j > 0 {
                visit(i, j - 1, k, &mut stack);
            }
            if j + 1 < n[1] {
                visit(i, j + 1, k, &mut stack);
            }
            if k > 0 {
                visit(i, j, k - 1, &mut stack);
            }
            if k + 1 < n[2] {
                visit(i, j, k + 1, &mut stack);
            }
        }
        // A driver fills its hole; a duct's mouth is air again once the outside is known.
        let mut sealed = vec![false; cells.len()];
        for &(cell, was, seal) in &held {
            if seal {
                cells[cell] = SOLID;
                sealed[cell] = true;
            } else {
                cells[cell] = if was == OUTSIDE || was == BLOCK {
                    AIR
                } else {
                    was
                };
            }
        }
        // Each enclosed air space gets a number, when the model marks holes.
        let mut labels = vec![0u32; cells.len()];
        let mut region_count = 0u32;
        if !found.is_empty() {
            for start in 0..cells.len() {
                if labels[start] != 0 || !matches!(cells[start], AIR | DAMPING) {
                    continue;
                }
                region_count += 1;
                labels[start] = region_count;
                let mut stack = vec![start];
                while let Some(at) = stack.pop() {
                    let (i, j, k) = (at / (n[1] * n[2]), (at / n[2]) % n[1], at % n[2]);
                    let mut next = |a: usize, b: usize, c: usize| {
                        let to = index(a, b, c);
                        if labels[to] == 0 && matches!(cells[to], AIR | DAMPING) {
                            labels[to] = region_count;
                            stack.push(to);
                        }
                    };
                    if i > 0 {
                        next(i - 1, j, k);
                    }
                    if i + 1 < n[0] {
                        next(i + 1, j, k);
                    }
                    if j > 0 {
                        next(i, j - 1, k);
                    }
                    if j + 1 < n[1] {
                        next(i, j + 1, k);
                    }
                    if k > 0 {
                        next(i, j, k - 1);
                    }
                    if k + 1 < n[2] {
                        next(i, j, k + 1);
                    }
                }
            }
        }
        let mut region_m3 = vec![0.0; region_count as usize + 1];
        // Volumes from the fine columns, exact along each column.
        let cell_index = |p: [f64; 3]| -> Option<usize> {
            let i = ((p[0] - grid_origin[0]) / dx).floor();
            let j = ((p[1] - grid_origin[1]) / dx).floor();
            let k = ((p[2] - grid_origin[2]) / dx).floor();
            if i < 0.0 || j < 0.0 || k < 0.0 {
                return None;
            }
            let (i, j, k) = (i as usize, j as usize, k as usize);
            if i >= n[0] || j >= n[1] || k >= n[2] {
                return None;
            }
            Some(index(i, j, k))
        };
        let cell_at = |p: [f64; 3]| -> u8 { cell_index(p).map_or(OUTSIDE, |c| cells[c]) };
        // The space a run of air belongs to: its coarse cell's, or a neighbour's when that
        // cell came out solid on the coarse grid.
        let label_near = |p: [f64; 3]| -> u32 {
            for d in [
                [0.0, 0.0, 0.0],
                [dx, 0.0, 0.0],
                [-dx, 0.0, 0.0],
                [0.0, dx, 0.0],
                [0.0, -dx, 0.0],
                [0.0, 0.0, dx],
                [0.0, 0.0, -dx],
            ] {
                if let Some(c) = cell_index([p[0] + d[0], p[1] + d[1], p[2] + d[2]]) {
                    if labels[c] != 0 {
                        return labels[c];
                    }
                }
            }
            0
        };
        let (mut air_m3, mut damping_m3) = (0.0, 0.0);
        // The exact air in each coarse cell, for measuring moulded ducts.
        let mut cell_air = if found.is_empty() {
            Vec::new()
        } else {
            vec![0.0f32; cells.len()]
        };
        let z_lo = grid_origin[2];
        let z_hi = grid_origin[2] + n[2] as f64 * dx;
        for i in 0..fine_n[0] {
            let x = grid_origin[0] + (i as f64 + 0.5) * fine_dx;
            for j in 0..fine_n[1] {
                let c = i * fine_n[1] + j;
                if fine.inside[c].is_empty() {
                    continue;
                }
                let y = grid_origin[1] + (j as f64 + 0.5) * fine_dx;
                // Every change of state along the column.
                let mut cuts: Vec<f64> = vec![z_lo, z_hi];
                for &(_, z0, z1) in &fine.inside[c] {
                    cuts.push(z0);
                    cuts.push(z1);
                }
                cuts.sort_by(f64::total_cmp);
                for pair in cuts.windows(2) {
                    let len = pair[1] - pair[0];
                    if len <= 0.0 {
                        continue;
                    }
                    let mid = 0.5 * (pair[0] + pair[1]);
                    let here = fine.state(&parts, c, mid);
                    if here == SOLID {
                        continue;
                    }
                    // Enclosed or not: the coarse grid's verdict at this run, looked up
                    // where the run is widest from solid if its middle cell is solid.
                    let mut at = [x, y, mid];
                    let mut verdict = cell_at(at);
                    if verdict == SOLID {
                        for z in [pair[0] + 0.25 * len, pair[1] - 0.25 * len] {
                            let v = cell_at([x, y, z]);
                            if v != SOLID {
                                verdict = v;
                                at = [x, y, z];
                                break;
                            }
                        }
                    }
                    if verdict == OUTSIDE {
                        continue;
                    }
                    // Air in a hole a driver fills is not the cabinet's.
                    if verdict == SOLID && cell_index(at).is_some_and(|c| sealed[c]) {
                        continue;
                    }
                    let v = len * fine_dx * fine_dx;
                    if here == DAMPING {
                        damping_m3 += v;
                    } else {
                        air_m3 += v;
                    }
                    if region_count > 0 {
                        region_m3[label_near(at) as usize] += v;
                    }
                    if !cell_air.is_empty() {
                        let k0 = ((pair[0] - z_lo) / dx).floor().max(0.0) as usize;
                        let k1 = (((pair[1] - z_lo) / dx).floor() as usize).min(n[2] - 1);
                        if let Some(base) = cell_index([x, y, z_lo + 0.5 * dx]) {
                            for k in k0..=k1 {
                                let (c0, c1) = (z_lo + k as f64 * dx, z_lo + (k + 1) as f64 * dx);
                                let overlap = pair[1].min(c1) - pair[0].max(c0);
                                if overlap > 0.0 {
                                    cell_air[base + k] += (overlap * fine_dx * fine_dx) as f32;
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut notes = vec![
            "Imported cabinet: the air inside is what the solid parts enclose, found on a grid \
             across the parts and exactly along it; damping parts are air the sound passes \
             through."
                .to_string(),
        ];
        for (part, open) in parts.iter().zip(&fine.open_parts) {
            if *open {
                notes.push(format!(
                    "{} is not a closed solid; where its surface has gaps its inside is guessed.",
                    part.name
                ));
            }
        }
        if min.iter().any(|v| v.abs() > 0.002) {
            notes.push(
                "The imported parts do not start at the origin; the listener is placed in \
                 front of the middle of the parts' front, assuming they do."
                    .into(),
            );
        }
        if air_m3 <= 0.0 {
            return Err(
                "the solid parts enclose no air: the cabinet must be closed all round, or each \
                 hole in it marked where a driver fills it or a port opens."
                    .into(),
            );
        }
        // Spaces numbered by size, largest first.
        let mut regions = None;
        let mut behind = vec![None; points.len()];
        let mut hole_depth_m = vec![None; points.len()];
        let mut openings = Vec::new();
        if region_count > 0 {
            let mut order: Vec<usize> = (1..=region_count as usize).collect();
            order.sort_by(|a, b| region_m3[*b].total_cmp(&region_m3[*a]));
            let mut renumber = vec![0u32; region_count as usize + 1];
            for (new, old) in order.iter().enumerate() {
                renumber[*old] = new as u32 + 1;
            }
            for label in labels.iter_mut() {
                *label = renumber[*label as usize];
            }
            let air = AirRegions {
                origin_m: grid_origin,
                dx_m: dx,
                n,
                labels,
                volumes_m3: order.iter().map(|old| region_m3[*old]).collect(),
            };
            for hole in &found {
                let point = &points[hole.point];
                hole_depth_m[hole.point] = Some(hole.layers as f64 * dx);
                let region = hole_region(&air, hole);
                behind[hole.point] = region;
                if point.hole == Some(Hole::Opening) {
                    let region = region.ok_or(format!(
                        "mounting point {}: no enclosed air lies behind its opening.",
                        point.id
                    ))?;
                    openings.push(measure_opening(
                        &air, hole, point, region, &cells, &cell_air,
                    ));
                }
            }
            notes.push(format!(
                "Imported cabinet with marked holes: {} enclosed air space{} found on a {:.0} mm \
                 grid.",
                air.volumes_m3.iter().filter(|v| **v > 1e-6).count(),
                if air.volumes_m3.iter().filter(|v| **v > 1e-6).count() == 1 {
                    ""
                } else {
                    "s"
                },
                dx * 1000.0
            ));
            regions = Some(std::sync::Arc::new(air));
        }
        Ok(MeshShape {
            parts,
            points,
            min,
            max,
            cells,
            grid_origin,
            grid_dx: dx,
            grid_n: n,
            air_m3,
            damping_m3,
            notes,
            regions,
            behind,
            hole_depth_m,
            openings,
        })
    }

    fn cell(&self, p: [f64; 3]) -> u8 {
        let n = self.grid_n;
        let f = |k: usize| ((p[k] - self.grid_origin[k]) / self.grid_dx).floor();
        let (i, j, k) = (f(0), f(1), f(2));
        if i < 0.0 || j < 0.0 || k < 0.0 {
            return OUTSIDE;
        }
        let (i, j, k) = (i as usize, j as usize, k as usize);
        if i >= n[0] || j >= n[1] || k >= n[2] {
            return OUTSIDE;
        }
        self.cells[(i * n[1] + j) * n[2] + k]
    }

    /// The air taken up by damping parts, m^3.
    pub fn damping_volume_m3(&self) -> f64 {
        self.damping_m3
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    fn point(&self, id: &str) -> Result<&Point, String> {
        if id.is_empty() {
            return Err(
                "this sound source is not placed on the model yet: choose Place on model and \
                 click where it goes."
                    .into(),
            );
        }
        self.points
            .iter()
            .find(|p| p.id == id)
            .ok_or(format!("the imported cabinet has no mounting point {id}."))
    }

    /// Distances along a ray to every solid triangle it crosses.
    fn ray_hits(&self, from: [f64; 3], dir: [f64; 3]) -> Vec<f64> {
        let mut hits = Vec::new();
        for part in self.parts.iter().filter(|p| !p.damping) {
            for t in &part.triangles {
                let [a, b, c] = t.map(|i| part.vertices[i as usize]);
                let e1 = sub(b, a);
                let e2 = sub(c, a);
                let h = cross(dir, e2);
                let det = dot(e1, h);
                if det.abs() < 1e-15 {
                    continue;
                }
                let s = sub(from, a);
                let u = dot(s, h) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = cross(s, e1);
                let v = dot(dir, q) / det;
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let t = dot(e2, q) / det;
                if t > 0.0 {
                    hits.push(t);
                }
            }
        }
        hits.sort_by(f64::total_cmp);
        hits
    }
}

impl OuterShape for MeshShape {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(self.air_m3 + self.damping_m3)
    }

    fn baffle_width_m(&self) -> f64 {
        self.max[0] - self.min[0]
    }

    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        Ok(self.max)
    }

    fn surfaces(&self) -> Vec<String> {
        self.points.iter().map(|p| p.id.clone()).collect()
    }

    fn mount_pose(&self, surface: &str, at_m: [f64; 2]) -> Result<MountPose, String> {
        let p = self.point(surface)?;
        Ok(MountPose {
            position_m: std::array::from_fn(|k| {
                p.position[k] + at_m[0] * p.t1[k] + at_m[1] * p.t2[k]
            }),
            outward: p.normal,
        })
    }

    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        self.points
            .iter()
            .max_by(|a, b| dot(a.normal, outward).total_cmp(&dot(b.normal, outward)))
            .filter(|p| dot(p.normal, outward) > 0.99)
            .map(|p| p.id.clone())
    }

    /// The flat face a mounting point sits on: every solid triangle in its plane, across all
    /// the parts (an assembly's halves, a front with a groove across it), and the convex
    /// outline of their corners.
    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        let p = self.point(surface)?;
        let d0 = dot(p.normal, p.position);
        let mut corners: Vec<[f64; 3]> = Vec::new();
        for part in self.parts.iter().filter(|part| !part.damping) {
            for t in &part.triangles {
                let [a, b, c] = t.map(|i| part.vertices[i as usize]);
                if unit(cross(sub(b, a), sub(c, a)))
                    .is_some_and(|n| dot(n, p.normal) > 0.999 && (dot(n, a) - d0).abs() < 5e-4)
                {
                    corners.extend([a, b, c]);
                }
            }
        }
        if corners.len() < 3 {
            return Err(format!(
                "mounting point {surface} is not on a flat area of a solid part."
            ));
        }
        // Convex hull in the surface's own coordinates (monotone chain).
        let mut pts: Vec<(f64, f64)> = corners
            .iter()
            .map(|c| {
                let r = sub(*c, p.position);
                (dot(r, p.t1), dot(r, p.t2))
            })
            .collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
        let turn = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
            (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
        };
        let mut hull: Vec<(f64, f64)> = Vec::new();
        for pass in 0..2 {
            let start = hull.len();
            let iter: Box<dyn Iterator<Item = &(f64, f64)>> = if pass == 0 {
                Box::new(pts.iter())
            } else {
                Box::new(pts.iter().rev())
            };
            for &q in iter {
                while hull.len() >= start + 2
                    && turn(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.0
                {
                    hull.pop();
                }
                hull.push(q);
            }
            hull.pop();
        }
        Ok(hull
            .into_iter()
            .map(|(u, v)| std::array::from_fn(|k| p.position[k] + u * p.t1[k] + v * p.t2[k]))
            .collect())
    }

    fn part_materials(&self) -> Vec<(String, String, f64, bool)> {
        self.parts
            .iter()
            .map(|p| (p.name.clone(), p.material.clone(), p.volume_m3, p.damping))
            .collect()
    }

    /// The flow resistivity at a point inside a damping part: the part's own figure, or that
    /// of its catalog material.
    fn damping_at(&self, p: [f64; 3]) -> Option<f64> {
        if self.cell(p) != DAMPING {
            return None;
        }
        // The damping part whose inside holds the point; the largest figure where two overlap.
        self.parts
            .iter()
            .filter(|part| part.damping)
            .map(|part| part.resistivity.unwrap_or(0.0))
            .reduce(f64::max)
    }

    fn contains_air(&self, point_m: [f64; 3]) -> Result<bool, String> {
        Ok(matches!(self.cell(point_m), AIR | DAMPING))
    }

    fn contains_solid(&self, point_m: [f64; 3]) -> Result<bool, String> {
        Ok(self.cell(point_m) == SOLID)
    }

    fn contains_cabinet(&self, point_m: [f64; 3]) -> Result<bool, String> {
        Ok(self.cell(point_m) != OUTSIDE)
    }

    /// The thickness of solid under a mounting point, along its inward direction.
    fn panel_thickness_m(&self, surface: &str) -> Result<f64, String> {
        let p = self.point(surface)?;
        // Over a marked hole the wall is measured beside it, on the grid.
        if let Some(depth) = self
            .points
            .iter()
            .position(|q| q.id == surface)
            .and_then(|i| self.hole_depth_m[i])
        {
            return Ok(depth);
        }
        let back = 0.002;
        let from: [f64; 3] = std::array::from_fn(|k| p.position[k] + back * p.normal[k]);
        let hits = self.ray_hits(from, [-p.normal[0], -p.normal[1], -p.normal[2]]);
        match hits.as_slice() {
            [enter, exit, ..] => Ok(exit - enter),
            _ => Err(format!(
                "no solid wall lies under mounting point {surface}."
            )),
        }
    }

    fn print_solid_m3(&self, cutouts: &[(String, f64)]) -> Result<f64, String> {
        let mut total: f64 = self
            .parts
            .iter()
            .filter(|p| !p.damping)
            .map(|p| p.volume_m3)
            .sum();
        // Each cut-out is a surface and the area of the hole through it.
        for (surface, area) in cutouts {
            total -= area * self.panel_thickness_m(surface)?;
        }
        Ok(total.max(0.0))
    }

    fn notes(&self) -> Vec<String> {
        self.notes.clone()
    }

    fn print_tree(&self, cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        let solids: Vec<Value> = self
            .parts
            .iter()
            .filter(|p| !p.damping)
            .map(|part| {
                json!({ "op": "mesh",
                    "vertices_m": part.vertices.iter().flatten().collect::<Vec<_>>(),
                    "triangles": part.triangles.iter().flatten().collect::<Vec<_>>() })
            })
            .collect();
        let mut parts = vec![csg_union(solids)];
        for (pose, diameter) in cutouts {
            let surface = self.surface_facing(pose.outward).unwrap_or_else(|| {
                self.points
                    .first()
                    .map(|p| p.id.clone())
                    .unwrap_or_default()
            });
            let thickness = self
                .points
                .iter()
                .filter(|p| dot(p.normal, pose.outward) > 0.99)
                .min_by(|a, b| {
                    let d = |q: &Point| {
                        let r = sub(q.position, pose.position_m);
                        dot(r, r)
                    };
                    d(a).total_cmp(&d(b))
                })
                .and_then(|p| self.panel_thickness_m(&p.id).ok())
                .or_else(|| self.panel_thickness_m(&surface).ok())
                .unwrap_or(0.02);
            parts.push(csg_hole(pose, *diameter, thickness));
        }
        Ok(csg_difference(parts))
    }

    fn air_tree(&self) -> Result<Value, String> {
        Err("an imported cabinet's air is found on a grid, not built from solids.".into())
    }

    fn air_regions(&self) -> Option<std::sync::Arc<AirRegions>> {
        self.regions.clone()
    }

    fn region_behind(&self, surface: &str) -> Option<usize> {
        let index = self.points.iter().position(|p| p.id == surface)?;
        self.behind[index]
    }

    fn moulded_openings(&self) -> Vec<MouldedOpening> {
        self.openings.clone()
    }
}

/// A marked hole found on the coarse grid, with the cells held closed while the outside
/// floods in.
struct HoleFound {
    found: FoundHole,
    blocked: Vec<usize>,
}

/// The open patch of surface under a marked point, layer by layer through the wall.
fn find_hole(
    cells: &[u8],
    n: [usize; 3],
    origin: [f64; 3],
    dx: f64,
    index: usize,
    point: &Point,
    hole: Hole,
) -> Result<HoleFound, String> {
    let axis = (0..3)
        .max_by(|a, b| point.normal[*a].abs().total_cmp(&point.normal[*b].abs()))
        .unwrap_or(0);
    if point.normal[axis].abs() < 0.99 {
        return Err(
            "a marked hole must be on a face that faces along one of the model's axes.".into(),
        );
    }
    let inward: isize = if point.normal[axis] > 0.0 { -1 } else { 1 };
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let at: [f64; 3] = std::array::from_fn(|k| point.position[k] - 0.5 * dx * point.normal[k]);
    let c0: [isize; 3] = std::array::from_fn(|k| ((at[k] - origin[k]) / dx).floor() as isize);
    let idx = |c: [isize; 3]| -> Option<usize> {
        if (0..3).any(|k| c[k] < 0 || c[k] >= n[k] as isize) {
            return None;
        }
        Some((c[0] as usize * n[1] + c[1] as usize) * n[2] + c[2] as usize)
    };
    let reach = (0.12 / dx).ceil() as isize;
    // The open cells of one layer joined to the cell under the point, within `allowed`.
    let flood = |layer: isize, allowed: Option<&std::collections::HashSet<(isize, isize)>>| {
        let mut seen = std::collections::HashSet::new();
        let mut open = Vec::new();
        let mut stack = vec![(0isize, 0isize)];
        let mut escaped = false;
        while let Some((a, b)) = stack.pop() {
            if !seen.insert((a, b)) {
                continue;
            }
            if a.abs() > reach || b.abs() > reach {
                escaped = true;
                continue;
            }
            if allowed.is_some_and(|set| !set.contains(&(a, b))) {
                continue;
            }
            let mut c = c0;
            c[axis] += inward * layer;
            c[u] += a;
            c[v] += b;
            let Some(cell) = idx(c) else { continue };
            if cells[cell] == SOLID {
                continue;
            }
            open.push(((a, b), cell));
            for (da, db) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                stack.push((a + da, b + db));
            }
        }
        (open, escaped)
    };
    let (mouth, escaped) = flood(0, None);
    if mouth.is_empty() {
        return Err("there is no hole under it.".into());
    }
    if escaped {
        return Err("the open area under it does not close within 12 cm of it.".into());
    }
    let footprint: std::collections::HashSet<(isize, isize)> =
        mouth.iter().map(|(ab, _)| *ab).collect();
    let mut grown = footprint.clone();
    for (a, b) in &footprint {
        for (da, db) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            grown.insert((a + da, b + db));
        }
    }
    // A driver fills its hole through the wall: layer by layer until the hole widens into the
    // space behind it (a quarter more open than its narrowest layer), at most 30 layers. A
    // duct's mouth is held closed by its first layer only.
    let mut blocked = Vec::new();
    let mut narrowest = usize::MAX;
    let mut layers = 0usize;
    let limit = if hole == Hole::Seal { 30 } else { 1 };
    while layers < limit {
        let (open, _) = flood(layers as isize, Some(&grown));
        if open.is_empty() {
            break;
        }
        if layers > 0 && open.len() * 4 > narrowest * 5 {
            break;
        }
        narrowest = narrowest.min(open.len());
        blocked.extend(open.iter().map(|(_, cell)| *cell));
        // The rim too, so the closure holds where the grid steps around the hole's edge.
        for (a, b) in grown.difference(&footprint) {
            let mut c = c0;
            c[axis] += inward * layers as isize;
            c[u] += a;
            c[v] += b;
            if let Some(cell) = idx(c) {
                if cells[cell] == SOLID {
                    blocked.push(cell);
                }
            }
        }
        layers += 1;
    }
    let layers = layers.max(1);
    Ok(HoleFound {
        found: FoundHole {
            point: index,
            axis,
            inward,
            mouth_cells: mouth.iter().map(|(_, cell)| *cell).collect(),
            layers,
        },
        blocked,
    })
}

/// The air space behind a hole: for a driver, past the wall it fills; for an opening, the
/// duct behind its mouth.
fn hole_region(air: &AirRegions, hole: &FoundHole) -> Option<usize> {
    let n = air.n;
    let stride = [n[1] * n[2], n[2], 1];
    let mut counts = std::collections::HashMap::new();
    for &cell in &hole.mouth_cells {
        for step in 0..=(hole.layers + 3) {
            let c = [cell / stride[0], (cell / stride[1]) % n[1], cell % n[2]];
            let along = c[hole.axis] as isize + hole.inward * step as isize;
            if along < 0 || along >= n[hole.axis] as isize {
                break;
            }
            let at = cell as isize + (along - c[hole.axis] as isize) * stride[hole.axis] as isize;
            let label = air.labels[at as usize];
            if label > 0 {
                *counts.entry(label as usize - 1).or_insert(0usize) += 1;
                break;
            }
        }
    }
    counts.into_iter().max_by_key(|(_, c)| *c).map(|(r, _)| r)
}

/// A moulded duct measured from its air (README 6.2): the distance of every cell of its space
/// from the mouth, along the shortest way through the air; each step of one cell in that
/// distance is one slice of the duct, its area the cells in it. The duct ends where the slices
/// suddenly widen into a chamber. The center line runs through the slices' middles, and the
/// areas are scaled so the duct holds the air its slices do.
fn measure_opening(
    air: &AirRegions,
    hole: &FoundHole,
    point: &Point,
    region: usize,
    cells: &[u8],
    cell_air: &[f32],
) -> MouldedOpening {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    #[derive(PartialEq)]
    struct Item(f64, usize);
    impl Eq for Item {}
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            other.0.total_cmp(&self.0)
        }
    }
    let n = air.n;
    let dx = air.dx_m;
    let label = region as u32 + 1;
    let center = |cell: usize| -> [f64; 3] {
        let c = [cell / (n[1] * n[2]), (cell / n[2]) % n[1], cell % n[2]];
        std::array::from_fn(|k| air.origin_m[k] + (c[k] as f64 + 0.5) * dx)
    };
    let mut distance = vec![f64::INFINITY; air.labels.len()];
    let mut heap = BinaryHeap::new();
    for &cell in &hole.mouth_cells {
        if air.labels[cell] == label {
            distance[cell] = 0.0;
            heap.push(Item(0.0, cell));
        }
    }
    let limit = 3.0;
    while let Some(Item(d, cell)) = heap.pop() {
        if d > distance[cell] || d > limit {
            continue;
        }
        let c = [
            (cell / (n[1] * n[2])) as isize,
            ((cell / n[2]) % n[1]) as isize,
            (cell % n[2]) as isize,
        ];
        for di in -1..=1isize {
            for dj in -1..=1isize {
                for dk in -1..=1isize {
                    if di == 0 && dj == 0 && dk == 0 {
                        continue;
                    }
                    let to = [c[0] + di, c[1] + dj, c[2] + dk];
                    if (0..3).any(|k| to[k] < 0 || to[k] >= n[k] as isize) {
                        continue;
                    }
                    let next = (to[0] as usize * n[1] + to[1] as usize) * n[2] + to[2] as usize;
                    if air.labels[next] != label {
                        continue;
                    }
                    let step = dx * ((di * di + dj * dj + dk * dk) as f64).sqrt();
                    if d + step < distance[next] {
                        distance[next] = d + step;
                        heap.push(Item(d + step, next));
                    }
                }
            }
        }
    }
    // Slices one cell deep in distance.
    let bins = (limit / dx) as usize + 1;
    let mut count = vec![0usize; bins];
    let mut sum = vec![[0.0f64; 3]; bins];
    for (cell, d) in distance.iter().enumerate() {
        if d.is_finite() {
            let b = (d / dx) as usize;
            if b < bins {
                count[b] += 1;
                let p = center(cell);
                for k in 0..3 {
                    sum[b][k] += p[k];
                }
            }
        }
    }
    let mut end = count.iter().rposition(|c| *c > 0).map_or(0, |e| e + 1);
    for b in 3..end {
        let mut seen: Vec<usize> = count[..b].to_vec();
        seen.sort_unstable();
        let median = seen[seen.len() / 2].max(1);
        if count[b] as f64 > 2.5 * median as f64 {
            end = b;
            break;
        }
    }
    let mut middles: Vec<[f64; 3]> = (0..end)
        .filter(|b| count[*b] > 0)
        .map(|b| std::array::from_fn(|k| sum[b][k] / count[b] as f64))
        .collect();
    let counts: Vec<f64> = (0..end)
        .filter(|b| count[*b] > 0)
        .map(|b| count[b] as f64)
        .collect();
    // Smooth the middles: the grid's steps would lengthen the line.
    if middles.len() > 2 {
        let raw = middles.clone();
        for i in 1..raw.len() - 1 {
            middles[i] = std::array::from_fn(|k| (raw[i - 1][k] + raw[i][k] + raw[i + 1][k]) / 3.0);
        }
    }
    // The mouth's middle on the outer surface.
    let mut mouth: [f64; 3] = [0.0; 3];
    for &cell in &hole.mouth_cells {
        let p = center(cell);
        for k in 0..3 {
            mouth[k] += p[k] / hole.mouth_cells.len() as f64;
        }
    }
    mouth[hole.axis] = point.position[hole.axis];
    let (mut r_lo, mut r_hi, mut u_lo, mut u_hi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &cell in &hole.mouth_cells {
        let p = center(cell);
        let d = sub(p, mouth);
        r_lo = r_lo.min(dot(d, point.t1));
        r_hi = r_hi.max(dot(d, point.t1));
        u_lo = u_lo.min(dot(d, point.t2));
        u_hi = u_hi.max(dot(d, point.t2));
    }
    let mut path = vec![mouth];
    path.extend(middles);
    let lengths: Vec<f64> = path
        .windows(2)
        .map(|w| dot(sub(w[1], w[0]), sub(w[1], w[0])).sqrt().max(1e-9))
        .collect();
    // The duct's air, exactly: its cells' air, and the air in the part-solid cells along its
    // walls (shared with whatever else of its space they touch).
    let in_duct: Vec<bool> = distance
        .iter()
        .map(|d| d.is_finite() && ((d / dx) as usize) < end)
        .collect();
    let mut volume = 0.0f64;
    for cell in 0..cells.len() {
        if in_duct[cell] {
            volume += cell_air[cell] as f64;
            continue;
        }
        if cells[cell] != SOLID || cell_air[cell] <= 0.0 {
            continue;
        }
        let c = [
            (cell / (n[1] * n[2])) as isize,
            ((cell / n[2]) % n[1]) as isize,
            (cell % n[2]) as isize,
        ];
        let (mut duct, mut other) = (0, 0);
        for (axis, step) in [(0, 1), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)] {
            let mut d = c;
            d[axis] += step;
            if (0..3).any(|k| d[k] < 0 || d[k] >= n[k] as isize) {
                continue;
            }
            let near = (d[0] as usize * n[1] + d[1] as usize) * n[2] + d[2] as usize;
            if in_duct[near] {
                duct += 1;
            } else if air.labels[near] == label {
                other += 1;
            }
        }
        if duct > 0 {
            volume += cell_air[cell] as f64 * duct as f64 / (duct + other) as f64;
        }
    }
    let raw: f64 = lengths.iter().zip(&counts).map(|(l, c)| l * c).sum();
    let scale = if raw > 0.0 { volume / raw } else { 0.0 };
    MouldedOpening {
        id: point.id.clone(),
        region,
        mouth: MountPose {
            position_m: mouth,
            outward: point.normal,
        },
        mouth_size_m: [r_hi - r_lo + dx, u_hi - u_lo + dx],
        right: point.t1,
        up: point.t2,
        path_m: path,
        areas_m2: counts.iter().map(|c| c * scale).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed box mesh from `lo` to `hi`, wound outward when `out` is true.
    pub fn box_mesh(lo: [f64; 3], hi: [f64; 3], out: bool) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let v: Vec<[f64; 3]> = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { lo[0] } else { hi[0] },
                    if i & 2 == 0 { lo[1] } else { hi[1] },
                    if i & 4 == 0 { lo[2] } else { hi[2] },
                ]
            })
            .collect();
        let mut t = vec![
            [0, 2, 1],
            [1, 2, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 1, 4],
            [1, 5, 4],
            [2, 6, 3],
            [3, 6, 7],
            [0, 4, 2],
            [2, 4, 6],
            [1, 3, 5],
            [3, 7, 5],
        ];
        if !out {
            for tri in &mut t {
                tri.swap(1, 2);
            }
        }
        (v, t)
    }

    fn encode(v: &[[f64; 3]], t: &[[u32; 3]]) -> (String, String) {
        let fb: Vec<u8> = v
            .iter()
            .flatten()
            .flat_map(|x| (*x as f32).to_le_bytes())
            .collect();
        let ib: Vec<u8> = t.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
        (encode_base64(&fb), encode_base64(&ib))
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..10 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
        }
    }

    #[test]
    fn a_hollow_box_holds_its_inside_and_a_damping_block_is_counted_apart() {
        // A box shell as one mesh: the outside wound outward and the inside wound inward.
        let (mut v, mut t) = box_mesh([0.0; 3], [0.16, 0.18, 0.25], true);
        let (vi, ti) = box_mesh([0.008; 3], [0.152, 0.172, 0.242], false);
        let off = v.len() as u32;
        v.extend(vi);
        t.extend(ti.iter().map(|x| x.map(|i| i + off)));
        let (sv, st) = encode(&v, &t);
        let (dv, dt) = encode(
            &box_mesh([0.05, 0.1, 0.05], [0.1, 0.15, 0.1], true).0,
            &box_mesh([0.05, 0.1, 0.05], [0.1, 0.15, 0.1], true).1,
        );
        let fields = json!({ "parts": [
            { "name": "shell", "role": "solid", "material": "pla", "vertices_m": sv, "triangles": st },
            { "name": "wool", "role": "damping", "vertices_m": dv, "triangles": dt } ],
            "points": [ { "id": "p1", "point_m": [0.08, 0.0, 0.15], "normal": [0.0, -1.0, 0.0] } ] });
        let s = MeshShape::new(fields.as_object().unwrap()).unwrap();
        let inside = 0.144 * 0.164 * 0.234;
        let damp = 0.05f64.powi(3);
        let total = s.interior_volume_m3().unwrap();
        assert!(
            (total - inside).abs() < 2e-3 * inside,
            "{total} against {inside}"
        );
        assert!(
            (s.damping_volume_m3() - damp).abs() < 0.01 * damp,
            "{}",
            s.damping_volume_m3()
        );
        let shell = 0.16 * 0.18 * 0.25 - inside;
        assert!((s.print_solid_m3(&[]).unwrap() - shell).abs() < 1e-6 * shell);
        assert!(s.contains_air([0.08, 0.05, 0.2]).unwrap());
        assert!(s.contains_solid([0.004, 0.09, 0.1]).unwrap());
        assert!(!s.contains_cabinet([0.08, -0.01, 0.1]).unwrap());
        assert!(s.damping_at([0.075, 0.125, 0.075]).is_some());
        let t = s.panel_thickness_m("p1").unwrap();
        assert!((t - 0.008).abs() < 1e-6, "{t}");
        let pose = s.mount_pose("p1", [0.01, 0.02]).unwrap();
        assert!(
            (pose.position_m[0] - 0.09).abs() < 1e-12 && (pose.position_m[2] - 0.17).abs() < 1e-12
        );
        let outline = s.surface_outline("p1").unwrap();
        assert_eq!(outline.len(), 4);
        assert!(outline.iter().all(|c| c[1].abs() < 1e-9));
    }

    #[test]
    fn a_shell_with_a_hole_encloses_nothing() {
        // Five walls of a box (no lid), each its own part: the air is not enclosed.
        let mut parts = Vec::new();
        for (lo, hi) in [
            ([0.0, 0.0, 0.0], [0.2, 0.2, 0.01]),
            ([0.0, 0.0, 0.0], [0.01, 0.2, 0.2]),
            ([0.19, 0.0, 0.0], [0.2, 0.2, 0.2]),
            ([0.0, 0.0, 0.0], [0.2, 0.01, 0.2]),
            ([0.0, 0.19, 0.0], [0.2, 0.2, 0.2]),
        ] {
            let (bv, bt) = box_mesh(lo, hi, true);
            let (sv, st) = encode(&bv, &bt);
            parts.push(json!({ "vertices_m": sv, "triangles": st }));
        }
        let fields = json!({ "parts": parts });
        assert!(MeshShape::new(fields.as_object().unwrap()).is_err());
    }
}
