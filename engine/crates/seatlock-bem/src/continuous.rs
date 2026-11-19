//! Continuous linear elements on a spherical triangle mesh.
//!
//! One pressure unknown per vertex; shape functions are the barycentric coordinates
//! pulled onto the sphere with [`sphere_map`]. Aimed at the high-`ka` rigid-sphere
//! gap that constant (flat or curved) and discontinuous linear leave open.

use nalgebra::{DMatrix, DVector, LU};
use num_complex::Complex64;
use seatlock_core::{C0, RHO0};

use crate::curved::{mesh_radius, sphere_map};
use crate::mesh::TriangleMesh;
use crate::{default_chief_points, dgreen_dn, green, GAUSS4, PI, TRI_GAUSS7};

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Regular ∫ G N_local and ∫ ∂G/∂n N_local over a spherical triangle.
fn integrate_shaped(
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
    radius: f64,
    xi: [f64; 3],
    k: f64,
    local: usize,
) -> (Complex64, Complex64) {
    let mut s = Complex64::new(0.0, 0.0);
    let mut d = Complex64::new(0.0, 0.0);
    for (w, uv) in TRI_GAUSS7 {
        let l = [1.0 - uv[0] - uv[1], uv[0], uv[1]];
        let (y, n, jac) = sphere_map(a, b, c, radius, l);
        let area_w = 0.5 * jac * w * l[local];
        s += green(k, dist(xi, y)) * area_w;
        d += dgreen_dn(k, xi, y, n) * area_w;
    }
    (s, d)
}

/// Duffy single-layer ∫ G N_local about vertex `local` of the spherical triangle.
fn self_single_from_vertex(
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
    radius: f64,
    xi: [f64; 3],
    k: f64,
    local: usize,
) -> Complex64 {
    // Parameter corners: vertex `local` is the Duffy origin; the other two span the element.
    let corners_uv = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
    let origin = corners_uv[local];
    let others: Vec<[f64; 2]> = (0..3)
        .filter(|&i| i != local)
        .map(|i| corners_uv[i])
        .collect();
    let p = others[0];
    let q = others[1];
    let ab = [p[0] - origin[0], p[1] - origin[1]];
    let ac = [q[0] - origin[0], q[1] - origin[1]];
    let area_uv = 0.5 * (ab[0] * ac[1] - ab[1] * ac[0]).abs();
    if area_uv < 1e-18 {
        return Complex64::new(0.0, 0.0);
    }
    let mut total = Complex64::new(0.0, 0.0);
    for &(wu, u) in &GAUSS4 {
        for &(wv, v) in &GAUSS4 {
            let uu = origin[0] + u * (p[0] - origin[0]) + u * v * (q[0] - p[0]);
            let vv = origin[1] + u * (p[1] - origin[1]) + u * v * (q[1] - p[1]);
            let l = [1.0 - uu - vv, uu, vv];
            if l[0] < -1e-9 || l[1] < -1e-9 || l[2] < -1e-9 {
                continue;
            }
            let (y, _, jac) = sphere_map(a, b, c, radius, l);
            let r = dist(xi, y);
            if r < 1e-15 {
                continue;
            }
            let weight = 2.0 * area_uv * u * wu * wv * jac * l[local];
            total += green(k, r) * weight;
        }
    }
    total
}

fn assemble_continuous_sphere(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    chief_points: &[[f64; 3]],
) -> Result<ContinuousSphereSystem, String> {
    if frequency_hz <= 0.0 {
        return Err("frequency must be positive".into());
    }
    let radius = mesh_radius(mesh)?;
    let n = mesh.vertices.len();
    let k = 2.0 * PI * frequency_hz / C0;
    let m = n + chief_points.len();
    let mut left = DMatrix::<Complex64>::zeros(m, n);
    let mut right = DMatrix::<Complex64>::zeros(m, n);

    for i in 0..n {
        let xi = mesh.vertices[i];
        for (tri_idx, tri) in mesh.triangles.iter().enumerate() {
            let _ = tri_idx;
            let (a, b, c) = (
                mesh.vertices[tri[0]],
                mesh.vertices[tri[1]],
                mesh.vertices[tri[2]],
            );
            let local_of = |vert: usize| -> Option<usize> {
                if tri[0] == vert {
                    Some(0)
                } else if tri[1] == vert {
                    Some(1)
                } else if tri[2] == vert {
                    Some(2)
                } else {
                    None
                }
            };
            let singular = local_of(i);
            for (local, &col) in tri.iter().enumerate() {
                let (s_ij, d_ij) = match singular {
                    Some(loc_i) if local == loc_i => {
                        // N_i = 1 at xi: Duffy single-layer; PV double-layer ≈ 0 on the patch.
                        (
                            self_single_from_vertex(a, b, c, radius, xi, k, local),
                            Complex64::new(0.0, 0.0),
                        )
                    }
                    Some(_) => {
                        // Other shape functions vanish at xi; G N and ∂G/∂n N are integrable
                        // with ordinary quadrature (N ~ r near the vertex).
                        integrate_shaped(a, b, c, radius, xi, k, local)
                    }
                    None => integrate_shaped(a, b, c, radius, xi, k, local),
                };
                right[(i, col)] += s_ij;
                left[(i, col)] += d_ij;
            }
        }
        left[(i, i)] -= Complex64::new(0.5, 0.0);
    }

    for (c_index, &point) in chief_points.iter().enumerate() {
        let row = n + c_index;
        for tri in &mesh.triangles {
            let (a, b, c) = (
                mesh.vertices[tri[0]],
                mesh.vertices[tri[1]],
                mesh.vertices[tri[2]],
            );
            for (local, &col) in tri.iter().enumerate() {
                let (s_ij, d_ij) = integrate_shaped(a, b, c, radius, point, k, local);
                left[(row, col)] += d_ij;
                right[(row, col)] += s_ij;
            }
        }
    }

    let (lu, s) = if chief_points.is_empty() {
        let lu = left.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("continuous sphere BEM matrix was singular".into());
        }
        (lu, right)
    } else {
        let m_adj = left.adjoint();
        let normal = &m_adj * &left;
        let s = &m_adj * &right;
        let lu = normal.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("continuous sphere BEM CHIEF matrix was singular".into());
        }
        (lu, s)
    };

    Ok(ContinuousSphereSystem {
        lu,
        s,
        k,
        frequency_hz,
        radius,
        n_vert: n,
    })
}

/// Continuous linear CBIE on a spherical icosphere (optional CHIEF).
pub struct ContinuousSphereSystem {
    lu: LU<Complex64, nalgebra::Dyn, nalgebra::Dyn>,
    s: DMatrix<Complex64>,
    pub k: f64,
    pub frequency_hz: f64,
    pub radius: f64,
    n_vert: usize,
}

impl ContinuousSphereSystem {
    pub fn assemble(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_continuous_sphere(mesh, frequency_hz, &default_chief_points(mesh))
    }

    pub fn assemble_cbie(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_continuous_sphere(mesh, frequency_hz, &[])
    }

    pub fn solve_q(&self, q: &[Complex64]) -> Result<Vec<Complex64>, String> {
        if q.len() != self.n_vert {
            return Err(format!(
                "q has {} entries for {} vertices",
                q.len(),
                self.n_vert
            ));
        }
        let mut rhs = DVector::<Complex64>::zeros(self.n_vert);
        for (i, row) in rhs.iter_mut().enumerate() {
            for (j, &qj) in q.iter().enumerate() {
                *row += self.s[(i, j)] * qj;
            }
        }
        let p = self
            .lu
            .solve(&rhs)
            .ok_or_else(|| "continuous sphere BEM matrix was singular".to_string())?;
        Ok(p.iter().copied().collect())
    }

    pub fn solve_velocity(&self, v_n: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let omega = 2.0 * PI * self.frequency_hz;
        let q: Vec<_> = v_n
            .iter()
            .map(|&v| Complex64::new(0.0, -omega * RHO0) * v)
            .collect();
        self.solve_q(&q)
    }
}

/// Rigid plane-wave total nodal pressures (continuous linear on the sphere).
pub fn rigid_plane_wave_surface_continuous(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    toward: [f64; 3],
) -> Result<Vec<Complex64>, String> {
    let system = ContinuousSphereSystem::assemble(mesh, frequency_hz)?;
    let k = system.k;
    let r = system.radius;
    let mut q = Vec::with_capacity(mesh.vertices.len());
    let mut p_inc = Vec::with_capacity(mesh.vertices.len());
    for &vert in &mesh.vertices {
        let n = [vert[0] / r, vert[1] / r, vert[2] / r];
        let phase = -(toward[0] * vert[0] + toward[1] * vert[1] + toward[2] * vert[2]) * k;
        let inc = Complex64::from_polar(1.0, phase);
        p_inc.push(inc);
        let cos = toward[0] * n[0] + toward[1] * n[1] + toward[2] * n[2];
        q.push(Complex64::new(0.0, k * cos) * inc);
    }
    let p_s = system.solve_q(&q)?;
    Ok(p_inc
        .into_iter()
        .zip(p_s)
        .map(|(inc, scattered)| inc + scattered)
        .collect())
}
