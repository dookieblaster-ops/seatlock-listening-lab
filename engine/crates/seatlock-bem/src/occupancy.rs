//! Occupancy remesh for outer shapes that are not star-convex from the bounding-box centre.
//!
//! Samples `contains_cabinet` on a regular grid and extracts a closed triangle skin from the
//! faces between inside and outside cells. Used when the AABB-seed pull (README 5.6) cannot
//! follow a re-entrant shell.

use crate::mesh::TriangleMesh;
use std::collections::HashMap;

/// True when every ray from `origin` stays inside until it leaves once (star-convex).
pub fn is_star_convex_from<F>(
    origin: [f64; 3],
    size_m: [f64; 3],
    samples: usize,
    mut inside: F,
) -> Result<bool, String>
where
    F: FnMut([f64; 3]) -> Result<bool, String>,
{
    if !inside(origin)? {
        return Ok(false);
    }
    let scale = size_m.iter().copied().fold(0.0_f64, f64::max).max(1e-6);
    let radius = 0.5 * (size_m[0].hypot(size_m[1]).hypot(size_m[2])) + scale * 0.05;
    let n = samples.max(12);
    let mut dirs: Vec<[f64; 3]> = vec![
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    for i in 0..n {
        let y = 1.0 - (i as f64) / (n as f64 - 1.0).max(1.0) * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden * i as f64;
        dirs.push([r * theta.cos(), y, r * theta.sin()]);
    }
    let steps = 48;
    for dir in dirs {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if len < 1e-15 {
            continue;
        }
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let mut was_inside = true;
        let mut left = false;
        for s in 1..=steps {
            let t = (s as f64) / (steps as f64) * radius;
            let p = [
                origin[0] + d[0] * t,
                origin[1] + d[1] * t,
                origin[2] + d[2] * t,
            ];
            let now = inside(p)?;
            if was_inside && !now {
                left = true;
            } else if left && now {
                return Ok(false);
            }
            was_inside = now;
        }
    }
    Ok(true)
}

struct DualGrid {
    origin: [f64; 3],
    dx: f64,
    dy: f64,
    dz: f64,
}

fn corner_index(
    grid: &DualGrid,
    key: (usize, usize, usize),
    vertices: &mut Vec<[f64; 3]>,
    map: &mut HashMap<(usize, usize, usize), usize>,
) -> usize {
    if let Some(&existing) = map.get(&key) {
        return existing;
    }
    let p = [
        grid.origin[0] + key.0 as f64 * grid.dx,
        grid.origin[1] + key.1 as f64 * grid.dy,
        grid.origin[2] + key.2 as f64 * grid.dz,
    ];
    let index = vertices.len();
    vertices.push(p);
    map.insert(key, index);
    index
}

fn push_quad(
    grid: &DualGrid,
    triangles: &mut Vec<[usize; 3]>,
    vertices: &mut Vec<[f64; 3]>,
    map: &mut HashMap<(usize, usize, usize), usize>,
    corners: [(usize, usize, usize); 4],
    outward: [f64; 3],
) {
    let i00 = corner_index(grid, corners[0], vertices, map);
    let i10 = corner_index(grid, corners[1], vertices, map);
    let i11 = corner_index(grid, corners[2], vertices, map);
    let i01 = corner_index(grid, corners[3], vertices, map);
    for tri in [[i00, i10, i11], [i00, i11, i01]] {
        let a = vertices[tri[0]];
        let b = vertices[tri[1]];
        let c = vertices[tri[2]];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let align = n[0] * outward[0] + n[1] * outward[1] + n[2] * outward[2];
        if align < 0.0 {
            triangles.push([tri[0], tri[2], tri[1]]);
        } else {
            triangles.push(tri);
        }
    }
}

/// Closed outer skin from an occupancy grid over the axis-aligned bounding box.
///
/// Each face between an inside cell and an outside neighbour becomes two triangles, wound
/// so the normal points outward. `cells` is the count along the longest side; other axes
/// keep cubes roughly cubic. Dense O(N²) BEM wants this modest (`≈ 2 × bem_subdivisions`).
pub fn mesh_from_occupancy<F>(
    size_m: [f64; 3],
    cells: u32,
    mut inside: F,
) -> Result<TriangleMesh, String>
where
    F: FnMut([f64; 3]) -> Result<bool, String>,
{
    if size_m.iter().any(|&v| v <= 0.0) {
        return Err("box size must be positive".into());
    }
    let long = size_m.iter().copied().fold(0.0_f64, f64::max).max(1e-6);
    let n_long = cells.max(4) as usize;
    let h = long / n_long as f64;
    // One-cell pad so a cabinet that fills the AABB still has an outside neighbour.
    let pad = h;
    let origin = [-pad, -pad, -pad];
    let extent = [
        size_m[0] + 2.0 * pad,
        size_m[1] + 2.0 * pad,
        size_m[2] + 2.0 * pad,
    ];
    let nx = ((extent[0] / h).round() as usize).max(2);
    let ny = ((extent[1] / h).round() as usize).max(2);
    let nz = ((extent[2] / h).round() as usize).max(2);
    let dx = extent[0] / nx as f64;
    let dy = extent[1] / ny as f64;
    let dz = extent[2] / nz as f64;

    let mut grid = vec![false; nx * ny * nz];
    let cell = |i: usize, j: usize, k: usize| -> usize { (k * ny + j) * nx + i };
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let p = [
                    origin[0] + (i as f64 + 0.5) * dx,
                    origin[1] + (j as f64 + 0.5) * dy,
                    origin[2] + (k as f64 + 0.5) * dz,
                ];
                let in_box = p[0] >= 0.0
                    && p[0] <= size_m[0]
                    && p[1] >= 0.0
                    && p[1] <= size_m[1]
                    && p[2] >= 0.0
                    && p[2] <= size_m[2];
                grid[cell(i, j, k)] = in_box && inside(p)?;
            }
        }
    }
    if !grid.iter().any(|&v| v) {
        return Err(
            "The boundary-element remesh found no cabinet cells inside the bounding box.".into(),
        );
    }

    let dual = DualGrid { origin, dx, dy, dz };
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[usize; 3]> = Vec::new();
    let mut map: HashMap<(usize, usize, usize), usize> = HashMap::new();

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if !grid[cell(i, j, k)] {
                    continue;
                }
                if i == 0 || !grid[cell(i - 1, j, k)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [(i, j, k), (i, j + 1, k), (i, j + 1, k + 1), (i, j, k + 1)],
                        [-1.0, 0.0, 0.0],
                    );
                }
                if i + 1 == nx || !grid[cell(i + 1, j, k)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [
                            (i + 1, j, k),
                            (i + 1, j + 1, k),
                            (i + 1, j + 1, k + 1),
                            (i + 1, j, k + 1),
                        ],
                        [1.0, 0.0, 0.0],
                    );
                }
                if j == 0 || !grid[cell(i, j - 1, k)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [(i, j, k), (i + 1, j, k), (i + 1, j, k + 1), (i, j, k + 1)],
                        [0.0, -1.0, 0.0],
                    );
                }
                if j + 1 == ny || !grid[cell(i, j + 1, k)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [
                            (i, j + 1, k),
                            (i + 1, j + 1, k),
                            (i + 1, j + 1, k + 1),
                            (i, j + 1, k + 1),
                        ],
                        [0.0, 1.0, 0.0],
                    );
                }
                if k == 0 || !grid[cell(i, j, k - 1)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [(i, j, k), (i + 1, j, k), (i + 1, j + 1, k), (i, j + 1, k)],
                        [0.0, 0.0, -1.0],
                    );
                }
                if k + 1 == nz || !grid[cell(i, j, k + 1)] {
                    push_quad(
                        &dual,
                        &mut triangles,
                        &mut vertices,
                        &mut map,
                        [
                            (i, j, k + 1),
                            (i + 1, j, k + 1),
                            (i + 1, j + 1, k + 1),
                            (i, j + 1, k + 1),
                        ],
                        [0.0, 0.0, 1.0],
                    );
                }
            }
        }
    }

    if triangles.is_empty() {
        return Err(
            "The boundary-element remesh found no outer surface (the occupancy grid fills the \
             whole block)."
                .into(),
        );
    }
    TriangleMesh::from_triangles(vertices, triangles)
}
