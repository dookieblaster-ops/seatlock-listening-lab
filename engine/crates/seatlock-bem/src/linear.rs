//! Discontinuous linear triangular elements for exterior CBIE.
//!
//! Three collocation nodes per triangle at Marburg's optimal offset from the
//! edges (`λ = ½(1 − 1/√3)`). Shape functions `N_i = (L_i − λ)/(1 − 3λ)` are
//! discontinuous across edges, which suits sharp corners and improves high-`ka`
//! accuracy versus constant elements (CHK-BEM-6). Not the radiation default yet.

use nalgebra::{DMatrix, DVector, LU};
use num_complex::Complex64;
use seatlock_core::{C0, RHO0};

use crate::mesh::TriangleMesh;
use crate::{default_chief_points, dgreen_dn, green, GAUSS4, PI, TRI_GAUSS7};

/// Offset from a vertex toward the opposite edge for discontinuous linear nodes.
#[allow(clippy::excessive_precision)]
pub const LAMBDA: f64 = 0.5 * (1.0 - 0.577_350_269_189_625_8); // ½(1 − 1/√3)

const SHAPE_DEN: f64 = 1.0 - 3.0 * LAMBDA;

/// Barycentric coordinates of local node `local` (0, 1, or 2).
pub fn node_barycentric(local: usize) -> [f64; 3] {
    let mut l = [LAMBDA; 3];
    l[local] = 1.0 - 2.0 * LAMBDA;
    l
}

/// Shape function `N_local` at barycentric `l`.
pub fn shape(local: usize, l: [f64; 3]) -> f64 {
    (l[local] - LAMBDA) / SHAPE_DEN
}

fn point_bary(a: [f64; 3], b: [f64; 3], c: [f64; 3], l: [f64; 3]) -> [f64; 3] {
    [
        l[0] * a[0] + l[1] * b[0] + l[2] * c[0],
        l[0] * a[1] + l[1] * b[1] + l[2] * c[1],
        l[0] * a[2] + l[1] * b[2] + l[2] * c[2],
    ]
}

/// Collocation point for triangle `tri`, local node `local`.
pub fn collocation_point(mesh: &TriangleMesh, tri: usize, local: usize) -> [f64; 3] {
    let (a, b, c) = mesh.corners(tri);
    point_bary(a, b, c, node_barycentric(local))
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cx = ab[1] * ac[2] - ab[2] * ac[1];
    let cy = ab[2] * ac[0] - ab[0] * ac[2];
    let cz = ab[0] * ac[1] - ab[1] * ac[0];
    0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
}

/// Barycentric coords of `p` in triangle `(a,b,c)` (area coordinates).
fn bary_of(a: [f64; 3], b: [f64; 3], c: [f64; 3], p: [f64; 3]) -> [f64; 3] {
    let area = triangle_area(a, b, c).max(1e-30);
    let l0 = triangle_area(p, b, c) / area;
    let l1 = triangle_area(a, p, c) / area;
    let l2 = triangle_area(a, b, p) / area;
    [l0, l1, l2]
}

/// Regular (non-singular) ∫ G N_k and ∫ ∂G/∂n N_k over triangle `tri`.
fn integrate_shaped(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
    local_shape: usize,
) -> (Complex64, Complex64) {
    let (a, b, c) = mesh.corners(tri);
    let n = mesh.normals[tri];
    let mut s = Complex64::new(0.0, 0.0);
    let mut d = Complex64::new(0.0, 0.0);
    for (w, uv) in TRI_GAUSS7 {
        // TRI_GAUSS7 uses (u,v) with L_b=u, L_c=v, L_a=1-u-v.
        let l = [1.0 - uv[0] - uv[1], uv[0], uv[1]];
        let y = point_bary(a, b, c, l);
        let nk = shape(local_shape, l);
        let area_w = mesh.areas[tri] * w * nk;
        s += green(k, dist(xi, y)) * area_w;
        d += dgreen_dn(k, xi, y, n) * area_w;
    }
    (s, d)
}

/// Singular single-layer self term ∫ G N_k with Duffy about collocation `xi` on the flat triangle.
fn self_single_shaped(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
    local_shape: usize,
) -> Complex64 {
    let (a, b, c) = mesh.corners(tri);
    let mut total = Complex64::new(0.0, 0.0);
    for edge in [[a, b], [b, c], [c, a]] {
        total += duffy_green_shaped(xi, edge[0], edge[1], [a, b, c], k, local_shape);
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn duffy_green_shaped(
    xi: [f64; 3],
    p: [f64; 3],
    q: [f64; 3],
    corners: [[f64; 3]; 3],
    k: f64,
    local_shape: usize,
) -> Complex64 {
    let [a, b, c] = corners;
    let area = triangle_area(xi, p, q);
    if area < 1e-18 {
        return Complex64::new(0.0, 0.0);
    }
    let mut sum = Complex64::new(0.0, 0.0);
    for &(wu, u) in &GAUSS4 {
        for &(wv, v) in &GAUSS4 {
            let y = [
                xi[0] + u * (p[0] - xi[0]) + u * v * (q[0] - p[0]),
                xi[1] + u * (p[1] - xi[1]) + u * v * (q[1] - p[1]),
                xi[2] + u * (p[2] - xi[2]) + u * v * (q[2] - p[2]),
            ];
            let r = dist(xi, y);
            if r < 1e-15 {
                continue;
            }
            let l = bary_of(a, b, c, y);
            let nk = shape(local_shape, l);
            let weight = 2.0 * area * u * wu * wv * nk;
            sum += green(k, r) * weight;
        }
    }
    sum
}

fn dofs(mesh: &TriangleMesh) -> usize {
    mesh.triangles.len() * 3
}

fn assemble_linear_chief(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    chief_points: &[[f64; 3]],
) -> Result<LinearNeumannSystem, String> {
    if frequency_hz <= 0.0 {
        return Err("frequency must be positive".into());
    }
    let n_tri = mesh.triangles.len();
    let n = dofs(mesh);
    let k = 2.0 * PI * frequency_hz / C0;
    let m = n + chief_points.len();
    let mut left = DMatrix::<Complex64>::zeros(m, n);
    let mut right = DMatrix::<Complex64>::zeros(m, n);

    for tri_i in 0..n_tri {
        for local_i in 0..3 {
            let row = tri_i * 3 + local_i;
            let xi = collocation_point(mesh, tri_i, local_i);
            for tri_j in 0..n_tri {
                for local_j in 0..3 {
                    let col = tri_j * 3 + local_j;
                    let (s_ij, d_ij) = if tri_i == tri_j {
                        // Flat self: double-layer integral ≈ 0; Duffy single-layer.
                        (
                            self_single_shaped(mesh, tri_j, xi, k, local_j),
                            Complex64::new(0.0, 0.0),
                        )
                    } else {
                        integrate_shaped(mesh, tri_j, xi, k, local_j)
                    };
                    right[(row, col)] = s_ij;
                    left[(row, col)] = d_ij;
                    if tri_i == tri_j && local_i == local_j {
                        left[(row, col)] -= Complex64::new(0.5, 0.0);
                    }
                }
            }
        }
    }

    for (c_index, &point) in chief_points.iter().enumerate() {
        let row = n + c_index;
        for tri_j in 0..n_tri {
            for local_j in 0..3 {
                let col = tri_j * 3 + local_j;
                let (s_ij, d_ij) = integrate_shaped(mesh, tri_j, point, k, local_j);
                left[(row, col)] = d_ij;
                right[(row, col)] = s_ij;
            }
        }
    }

    let (lu, s) = if chief_points.is_empty() {
        let lu = left.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("linear BEM matrix was singular (irregular frequency or bad mesh)".into());
        }
        (lu, right)
    } else {
        let m_adj = left.adjoint();
        let normal = &m_adj * &left;
        let s = &m_adj * &right;
        let lu = normal.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("linear BEM CHIEF normal matrix was singular (bad mesh)".into());
        }
        (lu, s)
    };

    Ok(LinearNeumannSystem {
        lu,
        s,
        k,
        frequency_hz,
        n_tri,
    })
}

/// Factorized discontinuous-linear exterior Neumann CBIE (optional CHIEF).
pub struct LinearNeumannSystem {
    lu: LU<Complex64, nalgebra::Dyn, nalgebra::Dyn>,
    s: DMatrix<Complex64>,
    pub k: f64,
    pub frequency_hz: f64,
    n_tri: usize,
}

impl LinearNeumannSystem {
    /// Assemble with CHIEF interior points.
    pub fn assemble(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_linear_chief(mesh, frequency_hz, &default_chief_points(mesh))
    }

    /// Assemble plain CBIE (no CHIEF).
    pub fn assemble_cbie(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_linear_chief(mesh, frequency_hz, &[])
    }

    /// Surface nodal pressures from prescribed nodal `q` (length `3 * n_tri`).
    pub fn solve_q(&self, q: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let n = self.n_tri * 3;
        if q.len() != n {
            return Err(format!("q has {} entries for {n} linear DOFs", q.len()));
        }
        let mut rhs = DVector::<Complex64>::zeros(n);
        for (i, row) in rhs.iter_mut().enumerate() {
            for (j, &qj) in q.iter().enumerate() {
                *row += self.s[(i, j)] * qj;
            }
        }
        let p = self
            .lu
            .solve(&rhs)
            .ok_or_else(|| "linear BEM matrix was singular".to_string())?;
        Ok(p.iter().copied().collect())
    }

    /// Nodal pressures from outward normal velocity sampled at each collocation node.
    pub fn solve_velocity(&self, v_n: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let n = self.n_tri * 3;
        if v_n.len() != n {
            return Err(format!("v_n has {} entries for {n} linear DOFs", v_n.len()));
        }
        let omega = 2.0 * PI * self.frequency_hz;
        let q: Vec<_> = v_n
            .iter()
            .map(|&v| Complex64::new(0.0, -omega * RHO0) * v)
            .collect();
        self.solve_q(&q)
    }
}

/// Mean pressure on triangle `tri` from its three nodal values (exact for linear).
pub fn triangle_mean_pressure(p_nodes: &[Complex64], tri: usize) -> Complex64 {
    let base = tri * 3;
    (p_nodes[base] + p_nodes[base + 1] + p_nodes[base + 2]) / 3.0
}

/// Area-weighted mean of triangle mean pressures.
pub fn area_mean_pressure(mesh: &TriangleMesh, p_nodes: &[Complex64]) -> Complex64 {
    let mut sum = Complex64::new(0.0, 0.0);
    let mut w = 0.0;
    for (tri, &a) in mesh.areas.iter().enumerate() {
        sum += triangle_mean_pressure(p_nodes, tri) * a;
        w += a;
    }
    sum / w
}

/// Sample a scalar field given per-triangle (e.g. centroid) value at the three nodes.
pub fn sample_constant_on_nodes(mesh: &TriangleMesh, per_tri: &[Complex64]) -> Vec<Complex64> {
    let mut out = Vec::with_capacity(dofs(mesh));
    for (tri, &v) in per_tri.iter().enumerate() {
        let _ = tri;
        out.push(v);
        out.push(v);
        out.push(v);
    }
    out
}

/// Rigid plane-wave surface pressure with discontinuous linear elements (total field).
pub fn rigid_plane_wave_surface_linear(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    toward: [f64; 3],
) -> Result<Vec<Complex64>, String> {
    let k = 2.0 * PI * frequency_hz / C0;
    let n = dofs(mesh);
    let mut q = Vec::with_capacity(n);
    let mut p_inc = Vec::with_capacity(n);
    for tri in 0..mesh.triangles.len() {
        let normal = mesh.normals[tri];
        for local in 0..3 {
            let point = collocation_point(mesh, tri, local);
            let phase = -(toward[0] * point[0] + toward[1] * point[1] + toward[2] * point[2]) * k;
            let inc = Complex64::from_polar(1.0, phase);
            p_inc.push(inc);
            let cos = toward[0] * normal[0] + toward[1] * normal[1] + toward[2] * normal[2];
            q.push(Complex64::new(0.0, k * cos) * inc);
        }
    }
    let p_s = LinearNeumannSystem::assemble(mesh, frequency_hz)?.solve_q(&q)?;
    Ok(p_inc
        .into_iter()
        .zip(p_s)
        .map(|(inc, scattered)| inc + scattered)
        .collect())
}
