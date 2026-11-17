//! Burton–Miller combination of CBIE and HBIE (exterior Neumann).
//!
//! Removes the irregular frequencies of the plain CBIE (interior Dirichlet
//! eigenvalues; first sphere mode at `ka = π`). Coupling `α = −j/k` with our
//! exterior CBIE `(D − ½I)p = Sq`, Green `e^{-jkr}/(4πr)`, and `e^{+jωt}`
//! (the sign that matches the pulsating-sphere residual for this operator layout).

use nalgebra::{DMatrix, DVector, LU};
use num_complex::Complex64;

use crate::mesh::TriangleMesh;
use crate::{dgreen_dn, green, PI};

/// `∂G/∂n_x` at observer `from` with outward normal `n_at_from`.
pub fn dgreen_dn_x(k: f64, from: [f64; 3], to: [f64; 3], n_at_from: [f64; 3]) -> Complex64 {
    let rvec = [from[0] - to[0], from[1] - to[1], from[2] - to[2]];
    let r = (rvec[0] * rvec[0] + rvec[1] * rvec[1] + rvec[2] * rvec[2]).sqrt();
    if r < 1e-15 {
        return Complex64::new(0.0, 0.0);
    }
    let g = green(k, r);
    let dr_dn = (rvec[0] * n_at_from[0] + rvec[1] * n_at_from[1] + rvec[2] * n_at_from[2]) / r;
    g * Complex64::new(-1.0 / r, -k) * dr_dn
}

/// Hypersingular kernel `∂²G/∂n_x ∂n_y`.
pub fn d2green_dndn(
    k: f64,
    from: [f64; 3],
    to: [f64; 3],
    n_at_from: [f64; 3],
    n_at_to: [f64; 3],
) -> Complex64 {
    let rvec = [from[0] - to[0], from[1] - to[1], from[2] - to[2]];
    let r2 = rvec[0] * rvec[0] + rvec[1] * rvec[1] + rvec[2] * rvec[2];
    let r = r2.sqrt();
    if r < 1e-15 {
        return Complex64::new(0.0, 0.0);
    }
    let nxny = n_at_from[0] * n_at_to[0] + n_at_from[1] * n_at_to[1] + n_at_from[2] * n_at_to[2];
    let rx = rvec[0] * n_at_from[0] + rvec[1] * n_at_from[1] + rvec[2] * n_at_from[2];
    let ry = rvec[0] * n_at_to[0] + rvec[1] * n_at_to[1] + rvec[2] * n_at_to[2];
    let phase = Complex64::from_polar(1.0, -k * r);
    let jkr = Complex64::new(0.0, k * r);
    let one = Complex64::new(1.0, 0.0);
    // e^{-jkr}/(4π r³) * [(1+jkr) n_x·n_y − (3+3jkr−k²r²)(R·n_x)(R·n_y)/r²]
    let term = (one + jkr) * nxny
        - (Complex64::new(3.0, 0.0) * (one + jkr) - Complex64::new(k * k * r2, 0.0))
            * (rx * ry / r2);
    phase * term / (4.0 * PI * r * r * r)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
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

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-30);
    [v[0] / n, v[1] / n, v[2] / n]
}

/// Hadamard finite part of `∫ 1/r³ dS` over a flat triangle with interior collocation `c`.
fn static_hypersingular_fp(c: [f64; 3], a: [f64; 3], b: [f64; 3], d: [f64; 3], n: [f64; 3]) -> f64 {
    let mut total = 0.0;
    for &(p, q) in &[(a, b), (b, d), (d, a)] {
        total += edge_fp_1_over_r3(c, p, q, n);
    }
    total
}

/// One edge's contribution to `FP ∫ 1/r³ dS = −∑ (sin α + sin β) / h`.
fn edge_fp_1_over_r3(c: [f64; 3], p: [f64; 3], q: [f64; 3], n: [f64; 3]) -> f64 {
    let edge = sub(q, p);
    let len = (edge[0] * edge[0] + edge[1] * edge[1] + edge[2] * edge[2]).sqrt();
    if len < 1e-18 {
        return 0.0;
    }
    let tangent = scale(edge, 1.0 / len);
    // In-plane outward normal (from triangle interior through the edge).
    let mut nu = cross(tangent, n);
    let mid = scale(add(p, q), 0.5);
    if dot(sub(c, mid), nu) > 0.0 {
        // nu points toward c; flip so it points outward.
        nu = scale(nu, -1.0);
    }
    let nu = normalize(nu);
    let h = dot(sub(p, c), nu);
    if h < 1e-14 {
        return 0.0;
    }
    let foot = add(c, scale(nu, h));
    let ra = dist(c, p);
    let rb = dist(c, q);
    if ra < 1e-14 || rb < 1e-14 {
        return 0.0;
    }
    // Angles at c from the perpendicular to each endpoint: sin = |endpoint−foot| / |endpoint−c|.
    let sin_a = (dist(p, foot) / ra).clamp(0.0, 1.0);
    let sin_b = (dist(q, foot) / rb).clamp(0.0, 1.0);
    -(sin_a + sin_b) / h
}

fn bary(a: [f64; 3], b: [f64; 3], c: [f64; 3], uv: [f64; 2]) -> [f64; 3] {
    let u = uv[0];
    let v = uv[1];
    let w = 1.0 - u - v;
    [
        w * a[0] + u * b[0] + v * c[0],
        w * a[1] + u * b[1] + v * c[1],
        w * a[2] + u * b[2] + v * c[2],
    ]
}

fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let cx = cross(ab, ac);
    0.5 * (cx[0] * cx[0] + cx[1] * cx[1] + cx[2] * cx[2]).sqrt()
}

/// Regular part of the coplanar hypersingular kernel: `[e^{-jkr}(1+jkr) − 1] / (4π r³)`.
fn hypersingular_regular(k: f64, r: f64) -> Complex64 {
    if r < 1e-15 {
        // Leading term ~ −k²/(8π r) after expansion; limit of r * kernel is fine under Duffy.
        return Complex64::new(0.0, 0.0);
    }
    let phase = Complex64::from_polar(1.0, -k * r);
    let one_jkr = Complex64::new(1.0, k * r);
    (phase * one_jkr - Complex64::new(1.0, 0.0)) / (4.0 * PI * r * r * r)
}

/// Hypersingular self-term on a flat triangle (collocation at the centroid).
fn self_hypersingular(mesh: &TriangleMesh, tri: usize, k: f64) -> Complex64 {
    let (a, b, c) = mesh.corners(tri);
    let xi = mesh.centroids[tri];
    let n = mesh.normals[tri];
    let static_fp = static_hypersingular_fp(xi, a, b, c, n) / (4.0 * PI);
    let mut regular = Complex64::new(0.0, 0.0);
    for edge in [[a, b], [b, c], [c, a]] {
        regular += duffy_hypersingular_regular(xi, edge[0], edge[1], k);
    }
    Complex64::new(static_fp, 0.0) + regular
}

fn duffy_hypersingular_regular(xi: [f64; 3], p: [f64; 3], q: [f64; 3], k: f64) -> Complex64 {
    let area = triangle_area(xi, p, q);
    if area < 1e-18 {
        return Complex64::new(0.0, 0.0);
    }
    let mut sum = Complex64::new(0.0, 0.0);
    for &(wu, u) in &super::GAUSS4 {
        for &(wv, v) in &super::GAUSS4 {
            let y = [
                xi[0] + u * (p[0] - xi[0]) + u * v * (q[0] - p[0]),
                xi[1] + u * (p[1] - xi[1]) + u * v * (q[1] - p[1]),
                xi[2] + u * (p[2] - xi[2]) + u * v * (q[2] - p[2]),
            ];
            let r = dist(xi, y);
            let weight = 2.0 * area * u * wu * wv;
            sum += hypersingular_regular(k, r) * weight;
        }
    }
    sum
}

fn integrate_layers(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    n_x: [f64; 3],
    k: f64,
) -> (Complex64, Complex64, Complex64, Complex64) {
    let (a, b, c) = mesh.corners(tri);
    let n_y = mesh.normals[tri];
    let mut s = Complex64::new(0.0, 0.0);
    let mut d = Complex64::new(0.0, 0.0);
    let mut kk = Complex64::new(0.0, 0.0);
    let mut nn = Complex64::new(0.0, 0.0);
    for (w, uv) in super::TRI_GAUSS7 {
        let y = bary(a, b, c, uv);
        let area_w = mesh.areas[tri] * w;
        s += green(k, dist(xi, y)) * area_w;
        d += dgreen_dn(k, xi, y, n_y) * area_w;
        kk += dgreen_dn_x(k, xi, y, n_x) * area_w;
        nn += d2green_dndn(k, xi, y, n_x, n_y) * area_w;
    }
    (s, d, kk, nn)
}

/// Factorized Burton–Miller system `(D − ½I + α N) p = (S + α (K + ½I)) q`.
pub struct BurtonMillerSystem {
    pub(crate) lu: LU<Complex64, nalgebra::Dyn, nalgebra::Dyn>,
    pub(crate) rhs_op: DMatrix<Complex64>,
    pub k: f64,
    pub frequency_hz: f64,
    pub alpha: Complex64,
}

impl BurtonMillerSystem {
    pub fn assemble(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        if frequency_hz <= 0.0 {
            return Err("frequency must be positive".into());
        }
        let n = mesh.triangles.len();
        let k = 2.0 * PI * frequency_hz / seatlock_core::C0;
        // α = −j/k (see module docs).
        let alpha = Complex64::new(0.0, -1.0) / k;
        let mut left = DMatrix::<Complex64>::zeros(n, n);
        let mut rhs_op = DMatrix::<Complex64>::zeros(n, n);
        for i in 0..n {
            let xi = mesh.centroids[i];
            let n_x = mesh.normals[i];
            for j in 0..n {
                if i == j {
                    let s_ii = super::self_single_layer(mesh, j, k);
                    let n_ii = self_hypersingular(mesh, j, k);
                    // Flat: D_ii = 0, K_ii = 0; CBIE free −½; HBIE free +½ on RHS.
                    left[(i, j)] = Complex64::new(-0.5, 0.0) + alpha * n_ii;
                    rhs_op[(i, j)] = s_ii + alpha * Complex64::new(0.5, 0.0);
                } else {
                    let (s_ij, d_ij, k_ij, n_ij) = integrate_layers(mesh, j, xi, n_x, k);
                    left[(i, j)] = d_ij + alpha * n_ij;
                    rhs_op[(i, j)] = s_ij + alpha * k_ij;
                }
            }
        }
        let lu = left.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("Burton–Miller matrix was singular (bad mesh)".into());
        }
        Ok(Self {
            lu,
            rhs_op,
            k,
            frequency_hz,
            alpha,
        })
    }

    pub fn solve_q(&self, q: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let n = self.rhs_op.nrows();
        if q.len() != n {
            return Err(format!("q has {} entries for {n} triangles", q.len()));
        }
        let mut rhs = DVector::<Complex64>::zeros(n);
        for (i, row) in rhs.iter_mut().enumerate() {
            for (j, &qj) in q.iter().enumerate() {
                *row += self.rhs_op[(i, j)] * qj;
            }
        }
        let p = self
            .lu
            .solve(&rhs)
            .ok_or_else(|| "Burton–Miller matrix was singular (bad mesh)".to_string())?;
        Ok(p.iter().copied().collect())
    }

    pub fn solve_velocity(&self, v_n: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let omega = 2.0 * PI * self.frequency_hz;
        let q: Vec<_> = v_n
            .iter()
            .map(|&v| Complex64::new(0.0, -omega * seatlock_core::RHO0) * v)
            .collect();
        self.solve_q(&q)
    }
}
