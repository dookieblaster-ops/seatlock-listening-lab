//! Exterior acoustic boundary-element model (README 2.6 / 12 D).
//!
//! Frequency-domain Helmholtz CBIE on a triangulated closed surface, with the lab's
//! `e^{+jωt}` convention and free-space Green function `e^{-jkr}/(4πr)` (same hankel-2
//! convention as CHK-RAD-3's closed-form sphere series).
//!
//! First slice: constant triangular elements, collocation at centroids, Neumann data
//! (normal velocity). Validated on a pulsating sphere (CHK-BEM-1). Burton–Miller / CHIEF
//! and joining the driver network come next; irregular frequencies stay rare below
//! `ka ≈ π`.

use nalgebra::{DMatrix, DVector};
use num_complex::Complex64;
use seatlock_core::{C0, RHO0};

mod mesh;
pub use mesh::{icosphere, TriangleMesh};

const PI: f64 = std::f64::consts::PI;

/// Free-space Green function `G = e^{-j k r} / (4 π r)` for `e^{+j ω t}`.
pub fn green(k: f64, r: f64) -> Complex64 {
    if r < 1e-15 {
        return Complex64::new(0.0, 0.0);
    }
    Complex64::from_polar(1.0 / (4.0 * PI * r), -k * r)
}

/// `∂G/∂n_y` with `n` the outward normal at `y` (into the air for exterior problems).
pub fn dgreen_dn(k: f64, from: [f64; 3], to: [f64; 3], n_at_to: [f64; 3]) -> Complex64 {
    let rvec = [from[0] - to[0], from[1] - to[1], from[2] - to[2]];
    let r = (rvec[0] * rvec[0] + rvec[1] * rvec[1] + rvec[2] * rvec[2]).sqrt();
    if r < 1e-15 {
        return Complex64::new(0.0, 0.0);
    }
    let g = green(k, r);
    // rvec = x - y; ∂r/∂n_y = (y - x) · n / r = -(x - y) · n / r.
    let dr_dn = -(rvec[0] * n_at_to[0] + rvec[1] * n_at_to[1] + rvec[2] * n_at_to[2]) / r;
    g * Complex64::new(-1.0 / r, -k) * dr_dn
}

/// Surface pressure from prescribed outward normal velocity `v_n` (m/s) on each triangle.
///
/// Assembles `(D − ½ I) p = S q` with outward normals into the air,
/// `D_ij = ∫_{E_j} ∂G/∂n_y (x_i, y) dS(y)`, `S_ij = ∫_{E_j} G dS`, and
/// `q = ∂p/∂n = −j ω ρ v_n` (`e^{+jωt}`). Checked against the pulsating-sphere closed form.
pub fn solve_neumann(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    v_n: &[f64],
) -> Result<Vec<Complex64>, String> {
    let n = mesh.triangles.len();
    if v_n.len() != n {
        return Err(format!("v_n has {} entries for {n} triangles", v_n.len()));
    }
    if frequency_hz <= 0.0 {
        return Err("frequency must be positive".into());
    }
    let omega = 2.0 * PI * frequency_hz;
    let k = omega / C0;
    let mut a = DMatrix::<Complex64>::zeros(n, n);
    let mut rhs = DVector::<Complex64>::zeros(n);
    for i in 0..n {
        let xi = mesh.centroids[i];
        for j in 0..n {
            let (s_ij, d_ij) = if i == j {
                (self_single_layer(mesh, j, k), Complex64::new(0.0, 0.0))
            } else {
                integrate_element(mesh, j, xi, k)
            };
            a[(i, j)] = d_ij;
            if i == j {
                a[(i, j)] -= Complex64::new(0.5, 0.0);
            }
            let q = Complex64::new(0.0, -omega * RHO0) * v_n[j];
            rhs[i] += s_ij * q;
        }
    }
    let p = a
        .lu()
        .solve(&rhs)
        .ok_or_else(|| "BEM matrix was singular (irregular frequency or bad mesh)".to_string())?;
    Ok(p.iter().copied().collect())
}

/// Pulsating sphere: uniform outward `v_n`, closed form `p = ρ c v_n · j ka / (1 + j ka)`.
pub fn pulsating_sphere_pressure(ka: f64, v_n: f64) -> Complex64 {
    let jka = Complex64::new(0.0, ka);
    RHO0 * C0 * v_n * jka / (Complex64::new(1.0, 0.0) + jka)
}

fn integrate_element(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
) -> (Complex64, Complex64) {
    let (a, b, c) = mesh.corners(tri);
    let n = mesh.normals[tri];
    let mut s = Complex64::new(0.0, 0.0);
    let mut d = Complex64::new(0.0, 0.0);
    for (w, uv) in TRI_GAUSS7 {
        let y = bary(a, b, c, uv);
        let area_w = mesh.areas[tri] * w;
        s += green(k, dist(xi, y)) * area_w;
        d += dgreen_dn(k, xi, y, n) * area_w;
    }
    (s, d)
}

/// Singular single-layer self term: ∫ G dS on the flat triangle (∂G/∂n vanishes).
fn self_single_layer(mesh: &TriangleMesh, tri: usize, k: f64) -> Complex64 {
    let (a, b, c) = mesh.corners(tri);
    let xi = mesh.centroids[tri];
    let mut total = Complex64::new(0.0, 0.0);
    for edge in [[a, b], [b, c], [c, a]] {
        total += duffy_green(xi, edge[0], edge[1], k);
    }
    total
}

/// Duffy map on triangle `(xi, p, q)` with singularity at `xi`.
fn duffy_green(xi: [f64; 3], p: [f64; 3], q: [f64; 3], k: f64) -> Complex64 {
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
            let weight = 2.0 * area * u * wu * wv;
            if r < 1e-15 {
                continue;
            }
            sum += green(k, r) * weight;
        }
    }
    sum
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

/// 4-point Gauss–Legendre on [0, 1].
#[allow(clippy::excessive_precision)]
const GAUSS4: [(f64, f64); 4] = [
    (0.173_927_422_568_726_9, 0.069_431_844_202_973_7),
    (0.326_072_577_431_273_1, 0.330_009_478_207_564_2),
    (0.326_072_577_431_273_1, 0.669_990_521_792_435_8),
    (0.173_927_422_568_726_9, 0.930_568_155_797_026_3),
];

/// Degree-5 / 7-point triangle rule; weights sum to 1.
#[allow(clippy::excessive_precision)]
const TRI_GAUSS7: [(f64, [f64; 2]); 7] = [
    (0.225, [1.0 / 3.0, 1.0 / 3.0]),
    (
        0.132_394_152_788_506_2,
        [0.059_615_871_781_466_0, 0.470_142_064_105_115_1],
    ),
    (
        0.132_394_152_788_506_2,
        [0.470_142_064_105_115_1, 0.059_615_871_781_466_0],
    ),
    (
        0.132_394_152_788_506_2,
        [0.470_142_064_105_115_1, 0.470_142_064_105_115_1],
    ),
    (
        0.125_939_180_544_683_2,
        [0.797_426_985_353_087_3, 0.101_286_507_323_456_3],
    ),
    (
        0.125_939_180_544_683_2,
        [0.101_286_507_323_456_3, 0.797_426_985_353_087_3],
    ),
    (
        0.125_939_180_544_683_2,
        [0.101_286_507_323_456_3, 0.101_286_507_323_456_3],
    ),
];
