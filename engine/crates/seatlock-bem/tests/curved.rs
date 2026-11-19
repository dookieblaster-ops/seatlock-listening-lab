//! Spherical-triangle CBIE (CHK-BEM-7): curved geometry does **not** close the high-ka gap.
//!
//! Flat constant / linear facets share ~2.6 dB lit error at ka=1 vs the smooth series.
//! Radial projection onto the true sphere (exact area, radial normals) keeps that gap;
//! continuous linear on the same skin does too. Geometry alone is not the fix.

use num_complex::Complex64;
use seatlock_bem::{
    icosphere, mesh_radius, pulsating_sphere_pressure, rigid_plane_wave_surface,
    rigid_plane_wave_surface_continuous, rigid_plane_wave_surface_sphere,
    rigid_sphere_surface_pressure, sphere_area, ContinuousSphereSystem, SphereNeumannSystem,
};
use seatlock_core::C0;
use std::f64::consts::PI;

fn err_db(got: Complex64, expected: Complex64) -> f64 {
    20.0 * (got.norm() / expected.norm()).log10().abs()
}

fn lit_err_flat(mesh: &seatlock_bem::TriangleMesh, freq: f64, a: f64, ka: f64) -> f64 {
    let toward = [0.0, -1.0, 0.0];
    let lit = [0.0, -a, 0.0];
    let expected_db = 20.0 * rigid_sphere_surface_pressure(ka, 0.0).norm().log10();
    let p = rigid_plane_wave_surface(mesh, freq, toward).unwrap();
    let mut sum = Complex64::new(0.0, 0.0);
    let mut w = 0.0;
    for (i, (pi, area)) in p.iter().zip(mesh.areas.iter()).enumerate() {
        let c = mesh.centroids[i];
        let d =
            ((c[0] - lit[0]).powi(2) + (c[1] - lit[1]).powi(2) + (c[2] - lit[2]).powi(2)).sqrt();
        if d < 0.35 * a {
            sum += *pi * *area;
            w += *area;
        }
    }
    (20.0 * (sum / w).norm().log10() - expected_db).abs()
}

fn lit_err_sphere(system: &SphereNeumannSystem, p: &[Complex64], a: f64, ka: f64) -> f64 {
    let lit = [0.0, -a, 0.0];
    let expected_db = 20.0 * rigid_sphere_surface_pressure(ka, 0.0).norm().log10();
    let mut sum = Complex64::new(0.0, 0.0);
    let mut w = 0.0;
    for (i, (pi, area)) in p.iter().zip(system.areas.iter()).enumerate() {
        let c = system.collocation[i];
        let d =
            ((c[0] - lit[0]).powi(2) + (c[1] - lit[1]).powi(2) + (c[2] - lit[2]).powi(2)).sqrt();
        if d < 0.35 * a {
            sum += *pi * *area;
            w += *area;
        }
    }
    (20.0 * (sum / w).norm().log10() - expected_db).abs()
}

#[test]
fn sphere_area_is_closer_to_4pi_r2_than_the_flat_facets() {
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    let radius = mesh_radius(&mesh).unwrap();
    assert!((radius - a).abs() < 1e-12);
    let flat: f64 = mesh.areas.iter().sum();
    let curved: f64 = (0..mesh.triangles.len())
        .map(|t| sphere_area(&mesh, t, radius))
        .sum();
    let exact = 4.0 * PI * a * a;
    eprintln!(
        "area flat {flat:.6} curved {curved:.6} exact {exact:.6} (errs {:.3}% / {:.3}%)",
        100.0 * (flat - exact).abs() / exact,
        100.0 * (curved - exact).abs() / exact
    );
    assert!((curved - exact).abs() < (flat - exact).abs());
    assert!((curved - exact).abs() / exact < 0.01);
}

#[test]
fn chk_bem_7_curved_sphere_keeps_monopole_and_places_rigid_limit() {
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    assert_eq!(mesh.triangles.len(), 320);

    // Monopole still holds with curved geometry.
    for &ka in &[0.5_f64, 1.0, 2.0] {
        let freq = ka * C0 / (2.0 * PI * a);
        let expected = pulsating_sphere_pressure(ka, 1.0);
        let v = vec![Complex64::new(1.0, 0.0); mesh.triangles.len()];
        let system = SphereNeumannSystem::assemble(&mesh, freq).unwrap();
        let p = system.solve_velocity(&v).unwrap();
        let mut sum = Complex64::new(0.0, 0.0);
        let mut w = 0.0;
        for (pi, area) in p.iter().zip(system.areas.iter()) {
            sum += *pi * *area;
            w += *area;
        }
        let e = err_db(sum / w, expected);
        eprintln!("curved pulsating ka={ka}: {e:.3} dB");
        assert!(e < 0.5, "ka={ka}: {e:.2} dB");
    }

    // Rigid sphere at ka=1: curved constant on the true skin still shares the ~2.6 dB
    // lit gap with flat facets — the limit is constant discontinuous field order, not
    // only chordal geometry. Area is exact (see sphere_area test); monopole holds.
    let ka = 1.0;
    let freq = ka * C0 / (2.0 * PI * a);
    let toward = [0.0, -1.0, 0.0];
    let flat_err = lit_err_flat(&mesh, freq, a, ka);
    let (system, p) = rigid_plane_wave_surface_sphere(&mesh, freq, toward).unwrap();
    let curved_err = lit_err_sphere(&system, &p, a, ka);
    eprintln!("ka=1 lit: flat {flat_err:.3} dB, curved {curved_err:.3} dB");
    assert!(flat_err > 2.0 && curved_err > 2.0);
    assert!(
        (flat_err - curved_err).abs() < 0.25,
        "curved constant should not close the ka=1 lit gap ({flat_err:.2} vs {curved_err:.2})"
    );
    // Same plateau under refine (matches the flat 80→5120 finding on main): not geometry alone.
    let fine = icosphere(a, 3).unwrap();
    assert_eq!(fine.triangles.len(), 1280);
    let flat_fine = lit_err_flat(&fine, freq, a, ka);
    let (sys_f, p_f) = rigid_plane_wave_surface_sphere(&fine, freq, toward).unwrap();
    let curved_fine = lit_err_sphere(&sys_f, &p_f, a, ka);
    eprintln!("ka=1 lit 1280: flat {flat_fine:.3} dB, curved {curved_fine:.3} dB");
    assert!(flat_fine > 2.0 && curved_fine > 2.0);
}

#[test]
fn continuous_linear_on_sphere_keeps_monopole_and_reports_rigid_lit() {
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    let n = mesh.vertices.len();

    for &ka in &[0.5_f64, 1.0] {
        let freq = ka * C0 / (2.0 * PI * a);
        let expected = pulsating_sphere_pressure(ka, 1.0);
        let v = vec![Complex64::new(1.0, 0.0); n];
        let p = ContinuousSphereSystem::assemble(&mesh, freq)
            .unwrap()
            .solve_velocity(&v)
            .unwrap();
        // Area-weight by one-third of each spherical triangle per vertex.
        let mut sum = Complex64::new(0.0, 0.0);
        let mut w = 0.0;
        for (t, tri) in mesh.triangles.iter().enumerate() {
            let area = sphere_area(&mesh, t, a) / 3.0;
            for &vert in tri {
                sum += p[vert] * area;
                w += area;
            }
        }
        let e = err_db(sum / w, expected);
        eprintln!("continuous pulsating ka={ka}: {e:.3} dB");
        assert!(e < 1.0, "ka={ka}: {e:.2} dB");
    }

    let ka = 1.0;
    let freq = ka * C0 / (2.0 * PI * a);
    let toward = [0.0, -1.0, 0.0];
    let lit = [0.0, -a, 0.0];
    let expected_db = 20.0 * rigid_sphere_surface_pressure(ka, 0.0).norm().log10();
    let p = rigid_plane_wave_surface_continuous(&mesh, freq, toward).unwrap();
    let mut sum = Complex64::new(0.0, 0.0);
    let mut w = 0.0;
    for (vert, pi) in mesh.vertices.iter().zip(p.iter()) {
        let d =
            ((vert[0] - lit[0]).powi(2) + (vert[1] - lit[1]).powi(2) + (vert[2] - lit[2]).powi(2))
                .sqrt();
        if d < 0.35 * a {
            sum += *pi;
            w += 1.0;
        }
    }
    let err = (20.0 * (sum / w).norm().log10() - expected_db).abs();
    eprintln!("continuous ka=1 lit: {err:.3} dB (flat constant ~2.6)");
    // Record the number; claim <0.75 only when it actually closes the gap.
    assert!(err.is_finite());
}
