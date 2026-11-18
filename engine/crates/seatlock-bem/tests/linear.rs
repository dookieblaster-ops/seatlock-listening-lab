//! Discontinuous linear CBIE (CHK-BEM-6): monopole and dipole on the same mesh as constant.
//!
//! Flat higher-order elements do not close the high-`ka` rigid-sphere gap against the
//! smooth-sphere series — that needs curved geometry. Linear is kept for later Burton–Miller
//! and as a better field basis on faceted cabinets.

use num_complex::Complex64;
use seatlock_bem::{
    area_mean_pressure, collocation_point, icosphere, oscillating_sphere_pressure,
    pulsating_sphere_pressure, rigid_plane_wave_surface, rigid_plane_wave_surface_linear,
    rigid_sphere_surface_pressure, sample_constant_on_nodes, triangle_mean_pressure,
    LinearNeumannSystem, LAMBDA,
};
use seatlock_core::C0;
use std::f64::consts::PI;

fn err_db(got: Complex64, expected: Complex64) -> f64 {
    20.0 * (got.norm() / expected.norm()).log10().abs()
}

fn dipole_rel_l1(mesh: &seatlock_bem::TriangleMesh, p_nodes: &[Complex64], ka: f64, u: f64) -> f64 {
    let a = 0.1;
    let mut num = 0.0;
    let mut den = 0.0;
    for tri in 0..mesh.triangles.len() {
        let area = mesh.areas[tri];
        let mut sum = Complex64::new(0.0, 0.0);
        let mut e_sum = Complex64::new(0.0, 0.0);
        for local in 0..3 {
            let point = collocation_point(mesh, tri, local);
            let cos = point[2] / a;
            sum += p_nodes[tri * 3 + local];
            e_sum += oscillating_sphere_pressure(ka, cos, u);
        }
        let got = sum / 3.0;
        let expected = e_sum / 3.0;
        num += (got - expected).norm() * area;
        den += expected.norm() * area;
    }
    num / den
}

#[test]
fn discontinuous_nodes_sit_inside_the_triangle() {
    assert!((LAMBDA - 0.211_324_865_405_187_1).abs() < 1e-12);
    const {
        assert!(LAMBDA > 0.0 && LAMBDA < 1.0 / 3.0);
    };
}

#[test]
fn chk_bem_6_linear_matches_monopole_and_dipole() {
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    assert_eq!(mesh.triangles.len(), 320);

    for &ka in &[0.5_f64, 1.0, 2.0] {
        let freq = ka * C0 / (2.0 * PI * a);
        let expected = pulsating_sphere_pressure(ka, 1.0);
        let v =
            sample_constant_on_nodes(&mesh, &vec![Complex64::new(1.0, 0.0); mesh.triangles.len()]);
        let p = LinearNeumannSystem::assemble(&mesh, freq)
            .unwrap()
            .solve_velocity(&v)
            .unwrap();
        let e = err_db(area_mean_pressure(&mesh, &p), expected);
        eprintln!("linear pulsating ka={ka}: {e:.3} dB");
        assert!(e < 0.5, "ka={ka}: {e:.2} dB");
    }

    for &ka in &[0.5_f64, 1.0, 2.0] {
        let freq = ka * C0 / (2.0 * PI * a);
        let u = 1.0;
        let mut v = Vec::with_capacity(mesh.triangles.len() * 3);
        for tri in 0..mesh.triangles.len() {
            for local in 0..3 {
                let point = collocation_point(&mesh, tri, local);
                v.push(Complex64::new(u * point[2] / a, 0.0));
            }
        }
        let p = LinearNeumannSystem::assemble(&mesh, freq)
            .unwrap()
            .solve_velocity(&v)
            .unwrap();
        let rel = dipole_rel_l1(&mesh, &p, ka, u);
        eprintln!("linear dipole ka={ka}: rel L1 {rel:.4}");
        assert!(rel < 0.03, "ka={ka}: rel L1 {rel:.4}");
    }
}

#[test]
fn flat_linear_and_constant_share_the_faceted_rigid_sphere_limit() {
    // Proves the ka≳1 gap is the faceted geometry vs the smooth series, not element order.
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    let ka = 1.0;
    let freq = ka * C0 / (2.0 * PI * a);
    let toward = [0.0, -1.0, 0.0];
    let lit = [0.0, -a, 0.0];
    let expected_db = 20.0 * rigid_sphere_surface_pressure(ka, 0.0).norm().log10();

    let const_p = rigid_plane_wave_surface(&mesh, freq, toward).unwrap();
    let mut c_sum = Complex64::new(0.0, 0.0);
    let mut c_w = 0.0;
    for (i, (p, area)) in const_p.iter().zip(mesh.areas.iter()).enumerate() {
        let c = mesh.centroids[i];
        let d =
            ((c[0] - lit[0]).powi(2) + (c[1] - lit[1]).powi(2) + (c[2] - lit[2]).powi(2)).sqrt();
        if d < 0.35 * a {
            c_sum += *p * *area;
            c_w += *area;
        }
    }
    let const_err = (20.0 * (c_sum / c_w).norm().log10() - expected_db).abs();

    let lin_p = rigid_plane_wave_surface_linear(&mesh, freq, toward).unwrap();
    let mut l_sum = Complex64::new(0.0, 0.0);
    let mut l_w = 0.0;
    for tri in 0..mesh.triangles.len() {
        let c = mesh.centroids[tri];
        let d =
            ((c[0] - lit[0]).powi(2) + (c[1] - lit[1]).powi(2) + (c[2] - lit[2]).powi(2)).sqrt();
        if d < 0.35 * a {
            l_sum += triangle_mean_pressure(&lin_p, tri) * mesh.areas[tri];
            l_w += mesh.areas[tri];
        }
    }
    let lin_err = (20.0 * (l_sum / l_w).norm().log10() - expected_db).abs();
    eprintln!("ka=1 lit faceted: constant {const_err:.3} dB, linear {lin_err:.3} dB vs smooth");
    assert!(const_err > 2.0 && lin_err > 2.0);
    assert!(
        (const_err - lin_err).abs() < 0.25,
        "flat linear should not beat the faceted limit ({const_err:.2} vs {lin_err:.2})"
    );
}
