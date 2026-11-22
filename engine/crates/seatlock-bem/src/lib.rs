//! Exterior acoustic boundary-element model (README 2.6 / 12 D).
//!
//! Frequency-domain Helmholtz CBIE on a triangulated closed surface, with the lab's
//! `e^{+jωt}` convention and free-space Green function `e^{-jkr}/(4πr)` (same hankel-2
//! convention as CHK-RAD-3's closed-form sphere series).
//!
//! Constant triangular elements, collocation at centroids. Validated on a pulsating sphere
//! (CHK-BEM-1), a rigid sphere in a plane wave at low `ka` (CHK-BEM-2), an oscillating
//! sphere Neumann dipole (CHK-BEM-3), and exterior field evaluation (CHK-BEM-4). Registered
//! as the `bem` radiation member. The cabinet mesh seeds the bounding box and pulls vertices
//! onto the true outer skin. Surface solves use the CBIE with CHIEF interior points so
//! irregular frequencies stay nonsingular on constant elements (Burton–Miller needs smoother
//! elements; the `BurtonMillerSystem` stays for monopole experiments). Discontinuous linear
//! triangles (`LinearNeumannSystem`, CHK-BEM-6) pass monopole and dipole. Spherical-triangle
//! constant elements (`SphereNeumannSystem`, CHK-BEM-7) and continuous linear on the sphere
//! (`ContinuousSphereSystem`) keep monopole accuracy and exact skin area, but the high-`ka`
//! rigid-sphere lit gap vs the smooth series (~2.6 dB at `ka = 1`) remains — not chordal
//! geometry alone. Not the radiation default. Mouth radiation impedances (self and mutual)
//! from the same surface solve join the waveguide network (CHK-BEM-8 / CHK-BEM-9). Mouths are
//! cut as true open apertures in the outer mesh (CHK-BEM-10), not rigid skin patches.

use nalgebra::{DMatrix, DVector, LU};
use num_complex::Complex64;
use seatlock_core::{C0, RHO0};

mod burton_miller;
mod continuous;
mod curved;
mod linear;
mod mesh;
pub use burton_miller::{d2green_dndn, dgreen_dn_x, BurtonMillerSystem};
pub use continuous::{rigid_plane_wave_surface_continuous, ContinuousSphereSystem};
pub use curved::{
    mesh_radius, rigid_plane_wave_surface_sphere, sphere_area, sphere_collocation, sphere_map,
    SphereNeumannSystem,
};
pub use linear::{
    area_mean_pressure, collocation_point, rigid_plane_wave_surface_linear,
    sample_constant_on_nodes, triangle_mean_pressure, LinearNeumannSystem, LAMBDA,
};
pub use mesh::{
    box_aabb, icosphere, mean_edge_length, mesh_from_aabb_seed, mesh_with_open_apertures,
    MouthAperture, TriangleMesh,
};

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

/// Factorized exterior Neumann CBIE `(D − ½ I) p = S q` at one frequency,
/// optionally overdetermined with CHIEF interior rows and solved by normal equations.
///
/// Reuse across many right-hand sides (one per surface source) at the same frequency.
pub struct NeumannSystem {
    lu: LU<Complex64, nalgebra::Dyn, nalgebra::Dyn>,
    /// Maps `q` to the right-hand side of the (possibly normal) equations.
    s: DMatrix<Complex64>,
    pub k: f64,
    pub frequency_hz: f64,
}

/// A few points inside the mesh's axis-aligned bounds (CHIEF).
pub fn default_chief_points(mesh: &TriangleMesh) -> Vec<[f64; 3]> {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }
    let center = [
        0.5 * (min[0] + max[0]),
        0.5 * (min[1] + max[1]),
        0.5 * (min[2] + max[2]),
    ];
    let half = [
        0.5 * (max[0] - min[0]),
        0.5 * (max[1] - min[1]),
        0.5 * (max[2] - min[2]),
    ];
    let mut points = vec![center];
    for axis in 0..3 {
        let delta = 0.2 * half[axis];
        if delta <= 1e-9 {
            continue;
        }
        let mut plus = center;
        plus[axis] += delta;
        points.push(plus);
        let mut minus = center;
        minus[axis] -= delta;
        points.push(minus);
    }
    points
}

fn assemble_cbie_chief(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    chief_points: &[[f64; 3]],
) -> Result<NeumannSystem, String> {
    if frequency_hz <= 0.0 {
        return Err("frequency must be positive".into());
    }
    let n = mesh.triangles.len();
    let k = 2.0 * PI * frequency_hz / C0;
    let m = n + chief_points.len();
    let mut left = DMatrix::<Complex64>::zeros(m, n);
    let mut right = DMatrix::<Complex64>::zeros(m, n);
    for i in 0..n {
        let xi = mesh.centroids[i];
        for j in 0..n {
            let (s_ij, d_ij) = if i == j {
                (self_single_layer(mesh, j, k), Complex64::new(0.0, 0.0))
            } else {
                integrate_element(mesh, j, xi, k)
            };
            right[(i, j)] = s_ij;
            left[(i, j)] = d_ij;
            if i == j {
                left[(i, j)] -= Complex64::new(0.5, 0.0);
            }
        }
    }
    for (c_index, &point) in chief_points.iter().enumerate() {
        let row = n + c_index;
        for j in 0..n {
            let (s_ij, d_ij) = integrate_element(mesh, j, point, k);
            // Interior point: no free-term jump. ∫ (p ∂G/∂n − G q) = 0.
            left[(row, j)] = d_ij;
            right[(row, j)] = s_ij;
        }
    }

    let (lu, s) = if chief_points.is_empty() {
        let lu = left.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("BEM matrix was singular (irregular frequency or bad mesh)".into());
        }
        (lu, right)
    } else {
        // Normal equations (M* M) p = M* R q for the stacked CHIEF system.
        let m_adj = left.adjoint();
        let normal = &m_adj * &left;
        let s = &m_adj * &right;
        let lu = normal.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("BEM CHIEF normal matrix was singular (bad mesh)".into());
        }
        (lu, s)
    };
    Ok(NeumannSystem {
        lu,
        s,
        k,
        frequency_hz,
    })
}

impl NeumannSystem {
    /// Assemble CBIE with CHIEF interior points (default radiation path).
    pub fn assemble(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_cbie_chief(mesh, frequency_hz, &default_chief_points(mesh))
    }

    /// Assemble the plain exterior CBIE `(D − ½ I) p = S q` (no CHIEF; singular at irregular frequencies).
    pub fn assemble_cbie(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_cbie_chief(mesh, frequency_hz, &[])
    }

    /// Surface pressure from prescribed outward `q = ∂p/∂n` on each triangle.
    pub fn solve_q(&self, q: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let n = self.s.nrows();
        if q.len() != n {
            return Err(format!("q has {} entries for {n} triangles", q.len()));
        }
        let mut rhs = DVector::<Complex64>::zeros(n);
        for (i, row) in rhs.iter_mut().enumerate() {
            for (j, &qj) in q.iter().enumerate() {
                *row += self.s[(i, j)] * qj;
            }
        }
        let p = self.lu.solve(&rhs).ok_or_else(|| {
            "BEM matrix was singular (irregular frequency or bad mesh)".to_string()
        })?;
        Ok(p.iter().copied().collect())
    }

    /// Surface pressure from complex outward normal velocity `v_n` (m/s).
    pub fn solve_velocity(&self, v_n: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let omega = 2.0 * PI * self.frequency_hz;
        let q: Vec<_> = v_n
            .iter()
            .map(|&v| Complex64::new(0.0, -omega * RHO0) * v)
            .collect();
        self.solve_q(&q)
    }
}

/// Surface pressure from prescribed outward `q = ∂p/∂n` on each triangle.
///
/// Assembles `(D − ½ I) p = S q` with outward normals into the air,
/// `D_ij = ∫_{E_j} ∂G/∂n_y (x_i, y) dS(y)`, `S_ij = ∫_{E_j} G dS`.
pub fn solve_neumann_q(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    q: &[Complex64],
) -> Result<Vec<Complex64>, String> {
    NeumannSystem::assemble(mesh, frequency_hz)?.solve_q(q)
}

/// Surface pressure from prescribed outward normal velocity `v_n` (m/s) on each triangle.
///
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
    let complex: Vec<_> = v_n.iter().map(|&v| Complex64::new(v, 0.0)).collect();
    NeumannSystem::assemble(mesh, frequency_hz)?.solve_velocity(&complex)
}

/// Exterior field pressure at a point outside the body.
///
/// With the outward normal into the air (the same normal as the CBIE), Green's identity
/// for the exterior is `p(x) = ∫_S (p ∂G/∂n_y − G q) dS`. The opposite sign matches the
/// magnitude and misses the phase by 180°.
pub fn field_pressure(
    mesh: &TriangleMesh,
    k: f64,
    p_surf: &[Complex64],
    q: &[Complex64],
    point_m: [f64; 3],
) -> Result<Complex64, String> {
    let n = mesh.triangles.len();
    if p_surf.len() != n || q.len() != n {
        return Err(format!(
            "surface fields have {} / {} entries for {n} triangles",
            p_surf.len(),
            q.len()
        ));
    }
    let mut total = Complex64::new(0.0, 0.0);
    for j in 0..n {
        let (s_j, d_j) = integrate_field_element(mesh, j, point_m, k);
        total += d_j * p_surf[j] - s_j * q[j];
    }
    Ok(total)
}

/// Pressure from unit volume velocity of prescribed triangle velocities, at one exterior point.
///
/// `v_n[j]` is outward normal velocity on triangle `j`; the volume velocity is
/// `Σ v_n[j] * area[j]`. Returns `p(point) / U` when the velocities are scaled so `U = 1`.
pub fn transfer_from_velocity(
    system: &NeumannSystem,
    mesh: &TriangleMesh,
    v_n: &[Complex64],
    point_m: [f64; 3],
) -> Result<Complex64, String> {
    let p_surf = system.solve_velocity(v_n)?;
    let omega = 2.0 * PI * system.frequency_hz;
    let q: Vec<_> = v_n
        .iter()
        .map(|&v| Complex64::new(0.0, -omega * RHO0) * v)
        .collect();
    field_pressure(mesh, system.k, &p_surf, &q, point_m)
}

/// Unit-amplitude plane wave traveling in direction `toward` (unit vector), `e^{+jωt}`:
/// `p = exp(−j k toward·r)`.
pub fn plane_wave(toward: [f64; 3], k: f64, point_m: [f64; 3]) -> Complex64 {
    let phase = -(toward[0] * point_m[0] + toward[1] * point_m[1] + toward[2] * point_m[2]) * k;
    Complex64::from_polar(1.0, phase)
}

/// Scattered surface pressure on a rigid body in a plane wave: `∂p_s/∂n = −∂p_inc/∂n`.
///
/// Returns total surface pressure `p_inc + p_s` at each triangle centroid.
pub fn rigid_plane_wave_surface(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    toward: [f64; 3],
) -> Result<Vec<Complex64>, String> {
    let k = 2.0 * PI * frequency_hz / C0;
    let mut q = Vec::with_capacity(mesh.triangles.len());
    let mut p_inc = Vec::with_capacity(mesh.triangles.len());
    for (centroid, normal) in mesh.centroids.iter().zip(mesh.normals.iter()) {
        let inc = plane_wave(toward, k, *centroid);
        p_inc.push(inc);
        // ∇p_inc = −j k toward · p_inc; q_s = −∂p_inc/∂n = j k (toward·n) p_inc.
        let cos = toward[0] * normal[0] + toward[1] * normal[1] + toward[2] * normal[2];
        q.push(Complex64::new(0.0, k * cos) * inc);
    }
    let p_s = solve_neumann_q(mesh, frequency_hz, &q)?;
    Ok(p_inc
        .into_iter()
        .zip(p_s)
        .map(|(inc, scattered)| inc + scattered)
        .collect())
}

/// Pulsating sphere: uniform outward `v_n`, closed form `p = ρ c v_n · j ka / (1 + j ka)`.
pub fn pulsating_sphere_pressure(ka: f64, v_n: f64) -> Complex64 {
    let jka = Complex64::new(0.0, ka);
    RHO0 * C0 * v_n * jka / (Complex64::new(1.0, 0.0) + jka)
}

/// Exterior field of a pulsating sphere of radius `a` at radial distance `r` (`r ≥ a`):
/// `p(r) = p(a) · (a/r) · e^{-j k (r − a)}`.
pub fn pulsating_sphere_field(ka: f64, a_m: f64, r_m: f64, v_n: f64) -> Complex64 {
    let surface = pulsating_sphere_pressure(ka, v_n);
    if r_m <= 0.0 || a_m <= 0.0 {
        return Complex64::new(0.0, 0.0);
    }
    let k = ka / a_m;
    surface * (a_m / r_m) * Complex64::from_polar(1.0, -k * (r_m - a_m))
}

/// Spherical Hankel of the second kind (same series as CHK-RAD-3).
fn hankel2(n: usize, x: f64) -> Complex64 {
    let mut sum = Complex64::new(0.0, 0.0);
    let mut term = 1.0_f64;
    for k in 0..=n {
        if k > 0 {
            term *= ((n + k) * (n + 1 - k)) as f64 / (k as f64 * 2.0 * x);
        }
        sum += Complex64::i().powi(k as i32) * term;
    }
    let first =
        Complex64::new(0.0, -1.0).powi(n as i32 + 1) * Complex64::from_polar(1.0 / x, x) * sum;
    first.conj()
}

fn hankel2_prime(n: usize, x: f64) -> Complex64 {
    if n == 0 {
        return -hankel2(1, x);
    }
    hankel2(n - 1, x) - hankel2(n, x) * ((n + 1) as f64 / x)
}

fn legendre(n: usize, x: f64) -> f64 {
    let (mut previous, mut current) = (1.0, x);
    if n == 0 {
        return 1.0;
    }
    for m in 1..n {
        let next = ((2 * m + 1) as f64 * x * current - m as f64 * previous) / (m + 1) as f64;
        previous = current;
        current = next;
    }
    current
}

/// Total pressure on a rigid sphere of radius `a` for a unit plane wave, at angle `theta`
/// from the direction the wave comes from (CHK-RAD-3 closed form):
/// `p = −j/(ka)^2 * sum (2m+1) j^m P_m(cos θ) / h_m'(ka)`.
pub fn rigid_sphere_surface_pressure(ka: f64, theta: f64) -> Complex64 {
    let terms = (ka + 12.0).ceil() as usize;
    let mut sum = Complex64::new(0.0, 0.0);
    for m in 0..terms {
        sum += Complex64::i().powi(m as i32) * ((2 * m + 1) as f64 * legendre(m, theta.cos()))
            / hankel2_prime(m, ka);
    }
    Complex64::new(0.0, -1.0) / (ka * ka) * sum
}

/// Rigid sphere oscillating along +z with surface velocity `U cos θ` (`e^{+jωt}`):
/// `p(a,θ) = −j ρ c U (h₁(ka)/h₁'(ka)) cos θ`.
pub fn oscillating_sphere_pressure(ka: f64, cos_theta: f64, velocity_m_s: f64) -> Complex64 {
    let ratio = hankel2(1, ka) / hankel2_prime(1, ka);
    -Complex64::new(0.0, 1.0) * RHO0 * C0 * velocity_m_s * ratio * cos_theta
}

fn integrate_element(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
) -> (Complex64, Complex64) {
    integrate_field_element(mesh, tri, xi, k)
}

/// Single- and double-layer integrals of element `tri` observed from `xi` (no self singularity).
fn integrate_field_element(
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
pub(crate) fn self_single_layer(mesh: &TriangleMesh, tri: usize, k: f64) -> Complex64 {
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
pub(crate) const GAUSS4: [(f64, f64); 4] = [
    (0.173_927_422_568_726_9, 0.069_431_844_202_973_7),
    (0.326_072_577_431_273_1, 0.330_009_478_207_564_2),
    (0.326_072_577_431_273_1, 0.669_990_521_792_435_8),
    (0.173_927_422_568_726_9, 0.930_568_155_797_026_3),
];

/// Degree-5 / 7-point triangle rule; weights sum to 1.
#[allow(clippy::excessive_precision)]
pub(crate) const TRI_GAUSS7: [(f64, [f64; 2]); 7] = [
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
