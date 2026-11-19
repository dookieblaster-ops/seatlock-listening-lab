//! Spherical-triangle (curved) constant elements for exterior CBIE.
//!
//! An icosphere's vertices already lie on the sphere, but flat facets cut chords.
//! This module maps each parameter triangle onto the true sphere by radial projection
//! of the barycentric combination of its corners (CHK-BEM-7). Skin area matches `4πR²`;
//! monopole holds; the high-`ka` rigid-sphere lit gap vs the smooth series does **not**
//! close — same ~2.6 dB at `ka = 1` as flat facets. Not the radiation default.

use nalgebra::{DMatrix, DVector, LU};
use num_complex::Complex64;
use seatlock_core::{C0, RHO0};

use crate::mesh::TriangleMesh;
use crate::{default_chief_points, dgreen_dn, green, GAUSS4, PI, TRI_GAUSS7};

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn norm3(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
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

/// Sphere radius inferred from the first vertex (icosphere vertices share one radius).
pub fn mesh_radius(mesh: &TriangleMesh) -> Result<f64, String> {
    let r = norm3(mesh.vertices[0]);
    if r <= 0.0 {
        return Err("mesh radius must be positive".into());
    }
    for v in &mesh.vertices {
        if (norm3(*v) - r).abs() > 1e-9 * r.max(1.0) {
            return Err("curved sphere elements need every vertex on one sphere".into());
        }
    }
    Ok(r)
}

/// Map parameter barycentric coords onto the sphere through corners `a,b,c`.
/// Returns `(point, outward unit normal, surface Jacobian vs flat parameter area)`.
pub fn sphere_map(
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
    radius: f64,
    l: [f64; 3],
) -> ([f64; 3], [f64; 3], f64) {
    let raw = [
        l[0] * a[0] + l[1] * b[0] + l[2] * c[0],
        l[0] * a[1] + l[1] * b[1] + l[2] * c[1],
        l[0] * a[2] + l[1] * b[2] + l[2] * c[2],
    ];
    let s = norm3(raw).max(1e-30);
    let unit = scale(raw, 1.0 / s);
    let point = scale(unit, radius);
    // Parameter (u,v) = (L1, L2), L0 = 1−u−v. ∂raw/∂u = b−a, ∂raw/∂v = c−a.
    let dru = sub(b, a);
    let drv = sub(c, a);
    let dunit_u = sub(scale(dru, 1.0 / s), scale(raw, dot(raw, dru) / (s * s * s)));
    let dunit_v = sub(scale(drv, 1.0 / s), scale(raw, dot(raw, drv) / (s * s * s)));
    let dyu = scale(dunit_u, radius);
    let dyv = scale(dunit_v, radius);
    let jac = norm3(cross(dyu, dyv));
    (point, unit, jac)
}

/// Collocation point: barycentre of the spherical triangle (equal barycentric weights).
pub fn sphere_collocation(mesh: &TriangleMesh, tri: usize, radius: f64) -> [f64; 3] {
    let (a, b, c) = mesh.corners(tri);
    sphere_map(a, b, c, radius, [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]).0
}

/// Spherical surface area of triangle `tri` (7-point quadrature on the projected map).
pub fn sphere_area(mesh: &TriangleMesh, tri: usize, radius: f64) -> f64 {
    let (a, b, c) = mesh.corners(tri);
    let mut area = 0.0;
    for (w, uv) in TRI_GAUSS7 {
        let l = [1.0 - uv[0] - uv[1], uv[0], uv[1]];
        let (_, _, jac) = sphere_map(a, b, c, radius, l);
        // TRI_GAUSS7 weights sum to 1 over the unit-area reference triangle of area 1/2,
        // matching flat `mesh.areas[tri] * w`. Here the parameter triangle has area
        // equal to the flat facet area, so scale jac by flat_area / (½) wait —
        // flat integrate uses `areas[tri] * w` with bary on the flat triangle whose
        // area is areas[tri]. The map's jac is |∂y/∂u × ∂y/∂v| for the same (u,v)
        // as the flat triangle with corners a,b,c, so dS = jac * du dv and
        // ∫ f dS ≈ Σ w_i * jac_i * (2 * flat_area) because Σ w = 1 over the
        // reference with ∫du dv = 1/2... Flat code: area_w = areas[tri] * w, and
        // flat jac = 2*area, so areas[tri]*w = jac_flat * (w/2) * 2? Standard:
        // for triangle area A, ∫f ≈ A Σ w_i f_i with Σ w_i = 1.
        // Here dS = jac_param du dv with ∫_{ref} du dv = 1/2 for unit right triangle,
        // but our (u,v) spans the actual triangle with ∫ du dv = flat_area?
        // Actually barycentric: y_flat = L0 a + L1 b + L2 c, ∫ g(L) dA_flat = A Σ w g.
        // For curved, dS = (jac / jac_flat) dA_flat ≈ (jac / (2A)) * ...
        // jac = |∂y/∂u × ∂y/∂v| with u=L1,v=L2; for flat y, jac_flat = 2A.
        // So dS = jac du dv, ∫du dv over triangle = A / (something)?
        // With L0=1-u-v, the domain is u≥0,v≥0,u+v≤1, area in (u,v) = 1/2.
        // Flat: |∂y/∂u × ∂y/∂v| = 2A, ∫ = ∫_ref 2A du dv = 2A * 1/2 = A. Good.
        // Curved: ∫ = ∫_ref jac du dv. Weights: TRI_GAUSS7 is for ∫_ref f * 2 du dv
        // style used as A*w with Σw=1, i.e. ∫_ref (2A f) du dv ≈ A Σ w f.
        // So ∫_ref jac f du dv ≈ Σ w * (jac/2) * f  when Σw=1 corresponds to
        // ∫_ref 2 f du dv ≈ Σ w f.
        // Therefore area ≈ Σ w * (jac / 2).
        area += w * 0.5 * jac;
    }
    area
}

fn integrate_sphere_element(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
    radius: f64,
) -> (Complex64, Complex64) {
    let (a, b, c) = mesh.corners(tri);
    let mut s = Complex64::new(0.0, 0.0);
    let mut d = Complex64::new(0.0, 0.0);
    for (w, uv) in TRI_GAUSS7 {
        let l = [1.0 - uv[0] - uv[1], uv[0], uv[1]];
        let (y, n, jac) = sphere_map(a, b, c, radius, l);
        let area_w = 0.5 * jac * w;
        s += green(k, dist(xi, y)) * area_w;
        d += dgreen_dn(k, xi, y, n) * area_w;
    }
    (s, d)
}

/// Singular single-layer on the spherical triangle via Duffy in parameter space.
fn self_single_sphere(
    mesh: &TriangleMesh,
    tri: usize,
    xi: [f64; 3],
    k: f64,
    radius: f64,
) -> Complex64 {
    let (a, b, c) = mesh.corners(tri);
    // Three sub-triangles from xi's parameter image. xi sits at bary (1/3,1/3,1/3)
    // on the sphere; Duffy about the parameter centroid of the flat reference.
    // Use the three corner fans in 3D along the curved surface by integrating
    // parameter Duffy from the collocation's parameter point (1/3,1/3,1/3).
    let xi_uv = [1.0 / 3.0, 1.0 / 3.0];
    let corners_uv = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
    let mut total = Complex64::new(0.0, 0.0);
    for edge in 0..3 {
        let p = corners_uv[edge];
        let q = corners_uv[(edge + 1) % 3];
        total += duffy_green_sphere(xi, xi_uv, p, q, a, b, c, radius, k);
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn duffy_green_sphere(
    xi: [f64; 3],
    xi_uv: [f64; 2],
    p: [f64; 2],
    q: [f64; 2],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
    radius: f64,
    k: f64,
) -> Complex64 {
    // Parameter-space area of the sub-triangle (xi_uv, p, q) in (u,v).
    let ab = [p[0] - xi_uv[0], p[1] - xi_uv[1]];
    let ac = [q[0] - xi_uv[0], q[1] - xi_uv[1]];
    let area_uv = 0.5 * (ab[0] * ac[1] - ab[1] * ac[0]).abs();
    if area_uv < 1e-18 {
        return Complex64::new(0.0, 0.0);
    }
    let mut sum = Complex64::new(0.0, 0.0);
    for &(wu, u) in &GAUSS4 {
        for &(wv, v) in &GAUSS4 {
            let uu = xi_uv[0] + u * (p[0] - xi_uv[0]) + u * v * (q[0] - p[0]);
            let vv = xi_uv[1] + u * (p[1] - xi_uv[1]) + u * v * (q[1] - p[1]);
            let l = [1.0 - uu - vv, uu, vv];
            if l[0] < -1e-9 || l[1] < -1e-9 || l[2] < -1e-9 {
                continue;
            }
            let (y, _, jac) = sphere_map(a, b, c, radius, l);
            let r = dist(xi, y);
            if r < 1e-15 {
                continue;
            }
            // Duffy: ∫_sub f |J| du dv with map (σ,τ)→ u=σ, edge blend; weight 2*area_uv*σ.
            // jac is |∂y/∂u × ∂y/∂v|; measure = jac du dv.
            let weight = 2.0 * area_uv * u * wu * wv * jac;
            sum += green(k, r) * weight;
        }
    }
    sum
}

fn assemble_sphere_chief(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    chief_points: &[[f64; 3]],
) -> Result<SphereNeumannSystem, String> {
    if frequency_hz <= 0.0 {
        return Err("frequency must be positive".into());
    }
    let radius = mesh_radius(mesh)?;
    let n = mesh.triangles.len();
    let k = 2.0 * PI * frequency_hz / C0;
    let m = n + chief_points.len();
    let mut left = DMatrix::<Complex64>::zeros(m, n);
    let mut right = DMatrix::<Complex64>::zeros(m, n);
    let mut areas = Vec::with_capacity(n);
    let mut collocation = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b, c) = mesh.corners(i);
        let (xi, nrm, _) = sphere_map(a, b, c, radius, [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
        collocation.push(xi);
        normals.push(nrm);
        areas.push(sphere_area(mesh, i, radius));
        for j in 0..n {
            let (s_ij, d_ij) = if i == j {
                // Smooth surface: free-term ½; PV double-layer on the small patch ≈ 0.
                (
                    self_single_sphere(mesh, j, xi, k, radius),
                    Complex64::new(0.0, 0.0),
                )
            } else {
                integrate_sphere_element(mesh, j, xi, k, radius)
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
            let (s_ij, d_ij) = integrate_sphere_element(mesh, j, point, k, radius);
            left[(row, j)] = d_ij;
            right[(row, j)] = s_ij;
        }
    }

    let (lu, s) = if chief_points.is_empty() {
        let lu = left.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("sphere BEM matrix was singular (irregular frequency or bad mesh)".into());
        }
        (lu, right)
    } else {
        let m_adj = left.adjoint();
        let normal = &m_adj * &left;
        let s = &m_adj * &right;
        let lu = normal.lu();
        let probe = DVector::from_element(n, Complex64::new(1.0, 0.0));
        if lu.solve(&probe).is_none() {
            return Err("sphere BEM CHIEF normal matrix was singular (bad mesh)".into());
        }
        (lu, s)
    };

    Ok(SphereNeumannSystem {
        lu,
        s,
        k,
        frequency_hz,
        radius,
        collocation,
        normals,
        areas,
    })
}

/// Factorized constant-field CBIE on spherical triangles (optional CHIEF).
pub struct SphereNeumannSystem {
    lu: LU<Complex64, nalgebra::Dyn, nalgebra::Dyn>,
    s: DMatrix<Complex64>,
    pub k: f64,
    pub frequency_hz: f64,
    pub radius: f64,
    pub collocation: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub areas: Vec<f64>,
}

impl SphereNeumannSystem {
    pub fn assemble(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_sphere_chief(mesh, frequency_hz, &default_chief_points(mesh))
    }

    pub fn assemble_cbie(mesh: &TriangleMesh, frequency_hz: f64) -> Result<Self, String> {
        assemble_sphere_chief(mesh, frequency_hz, &[])
    }

    pub fn solve_q(&self, q: &[Complex64]) -> Result<Vec<Complex64>, String> {
        let n = self.collocation.len();
        if q.len() != n {
            return Err(format!("q has {} entries for {n} triangles", q.len()));
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
            .ok_or_else(|| "sphere BEM matrix was singular".to_string())?;
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

/// Rigid plane-wave total surface pressure with spherical-triangle geometry.
pub fn rigid_plane_wave_surface_sphere(
    mesh: &TriangleMesh,
    frequency_hz: f64,
    toward: [f64; 3],
) -> Result<(SphereNeumannSystem, Vec<Complex64>), String> {
    let system = SphereNeumannSystem::assemble(mesh, frequency_hz)?;
    let k = system.k;
    let mut q = Vec::with_capacity(system.collocation.len());
    let mut p_inc = Vec::with_capacity(system.collocation.len());
    for (point, normal) in system.collocation.iter().zip(system.normals.iter()) {
        let phase = -(toward[0] * point[0] + toward[1] * point[1] + toward[2] * point[2]) * k;
        let inc = Complex64::from_polar(1.0, phase);
        p_inc.push(inc);
        let cos = toward[0] * normal[0] + toward[1] * normal[1] + toward[2] * normal[2];
        q.push(Complex64::new(0.0, k * cos) * inc);
    }
    let p_s = system.solve_q(&q)?;
    let total = p_inc
        .into_iter()
        .zip(p_s)
        .map(|(inc, scattered)| inc + scattered)
        .collect();
    Ok((system, total))
}
