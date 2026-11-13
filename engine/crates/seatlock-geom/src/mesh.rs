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

use seatlock_core::registry::{MountPose, OuterShape, Registry};
use serde_json::{json, Map, Value};

use crate::{csg_difference, csg_hole, csg_union};

/// What a place in space is.
const AIR: u8 = 0;
const DAMPING: u8 = 1;
const SOLID: u8 = 2;
const OUTSIDE: u8 = 3;

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

struct Point {
    id: String,
    position: [f64; 3],
    normal: [f64; 3],
    t1: [f64; 3],
    t2: [f64; 3],
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
        .chunks_exact(12)
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]) as f64;
            [f(0), f(4), f(8)]
        })
        .collect();
    let triangles: Vec<[u32; 3]> = ints
        .chunks_exact(12)
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
        resistivity: raw.get("flow_resistivity_pa_s_m2").and_then(Value::as_f64),
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
                for pair in zs.chunks_exact(2) {
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
        let coarse = columns(&parts, [grid_origin[0], grid_origin[1]], dx, n[0], n[1]);
        let mut cells = vec![AIR; n[0] * n[1] * n[2]];
        for i in 0..n[0] {
            for j in 0..n[1] {
                let c = i * n[1] + j;
                for k in 0..n[2] {
                    let z = grid_origin[2] + (k as f64 + 0.5) * dx;
                    cells[c * n[2] + k] = coarse.state(&parts, c, z);
                }
            }
        }
        // Flood the outside in from the corner, through everything that is not solid.
        let index = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
        let mut stack = vec![(0usize, 0usize, 0usize)];
        cells[0] = OUTSIDE;
        while let Some((i, j, k)) = stack.pop() {
            let mut visit = |a: usize, b: usize, c: usize, stack: &mut Vec<_>| {
                let at = index(a, b, c);
                if cells[at] != SOLID && cells[at] != OUTSIDE {
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
        // Volumes from fine columns, exact along each column.
        let fine_dx = dx / 4.0;
        let fine_n = [n[0] * 4, n[1] * 4];
        let fine = columns(
            &parts,
            [grid_origin[0], grid_origin[1]],
            fine_dx,
            fine_n[0],
            fine_n[1],
        );
        let cell_at = |p: [f64; 3]| -> u8 {
            let i = ((p[0] - grid_origin[0]) / dx).floor();
            let j = ((p[1] - grid_origin[1]) / dx).floor();
            let k = ((p[2] - grid_origin[2]) / dx).floor();
            if i < 0.0 || j < 0.0 || k < 0.0 {
                return OUTSIDE;
            }
            let (i, j, k) = (i as usize, j as usize, k as usize);
            if i >= n[0] || j >= n[1] || k >= n[2] {
                return OUTSIDE;
            }
            cells[index(i, j, k)]
        };
        let (mut air_m3, mut damping_m3) = (0.0, 0.0);
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
                    let mut verdict = cell_at([x, y, mid]);
                    if verdict == SOLID {
                        for z in [pair[0] + 0.25 * len, pair[1] - 0.25 * len] {
                            let v = cell_at([x, y, z]);
                            if v != SOLID {
                                verdict = v;
                                break;
                            }
                        }
                    }
                    if verdict == OUTSIDE {
                        continue;
                    }
                    let v = len * fine_dx * fine_dx;
                    if here == DAMPING {
                        damping_m3 += v;
                    } else {
                        air_m3 += v;
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
                "the solid parts enclose no air: the cabinet must be closed all round.".into(),
            );
        }
        let points = fields
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
                        Some(Point {
                            id,
                            position,
                            normal,
                            t1,
                            t2,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
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

    /// The flat area around a mounting point: the solid triangles in its plane joined to the
    /// one it sits on, and the convex outline of their corners.
    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        let p = self.point(surface)?;
        let d0 = dot(p.normal, p.position);
        let mut corners: Vec<[f64; 3]> = Vec::new();
        for part in self.parts.iter().filter(|part| !part.damping) {
            let flat: Vec<&[u32; 3]> = part
                .triangles
                .iter()
                .filter(|t| {
                    let [a, b, c] = t.map(|i| part.vertices[i as usize]);
                    unit(cross(sub(b, a), sub(c, a)))
                        .is_some_and(|n| dot(n, p.normal) > 0.999 && (dot(n, a) - d0).abs() < 5e-4)
                })
                .collect();
            if flat.is_empty() {
                continue;
            }
            // Grow from the triangle under the point through shared corners.
            let close = |a: [f64; 3], b: [f64; 3]| dot(sub(a, b), sub(a, b)) < 1e-12;
            let under = flat.iter().position(|t| {
                let [a, b, c] = t.map(|i| part.vertices[i as usize]);
                let w =
                    |u: [f64; 3], v: [f64; 3]| dot(cross(sub(v, u), sub(p.position, u)), p.normal);
                w(a, b) >= -1e-9 && w(b, c) >= -1e-9 && w(c, a) >= -1e-9
            });
            let Some(start) = under else { continue };
            let mut taken = vec![false; flat.len()];
            taken[start] = true;
            let mut region = vec![start];
            let mut k = 0;
            while k < region.len() {
                let here = flat[region[k]].map(|i| part.vertices[i as usize]);
                for (other, t) in flat.iter().enumerate() {
                    if taken[other] {
                        continue;
                    }
                    let there = t.map(|i| part.vertices[i as usize]);
                    if here.iter().any(|a| there.iter().any(|b| close(*a, *b))) {
                        taken[other] = true;
                        region.push(other);
                    }
                }
                k += 1;
            }
            for &r in &region {
                corners.extend(flat[r].map(|i| part.vertices[i as usize]));
            }
            break;
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
