//! Pulsating-sphere closed form for the exterior CBIE (CHK-BEM-1).

use seatlock_bem::{icosphere, pulsating_sphere_pressure, solve_neumann};
use seatlock_core::C0;

#[test]
fn the_pulsating_sphere_closed_form_has_the_right_limits() {
    let soft = pulsating_sphere_pressure(0.01, 1.0);
    assert!(soft.norm() < 0.02 * seatlock_core::RHO0 * C0);
    let hard = pulsating_sphere_pressure(40.0, 1.0);
    assert!((hard.norm() - seatlock_core::RHO0 * C0).abs() / (seatlock_core::RHO0 * C0) < 0.01);
}

#[test]
fn green_is_normalized_with_four_pi() {
    let g = seatlock_bem::green(0.0, 1.0);
    assert!((g.re - 1.0 / (4.0 * std::f64::consts::PI)).abs() < 1e-15);
}

#[test]
fn an_icosphere_orients_normals_outward() {
    let mesh = icosphere(0.2, 1).unwrap();
    for (i, n) in mesh.normals.iter().enumerate() {
        let c = mesh.centroids[i];
        assert!(
            n[0] * c[0] + n[1] * c[1] + n[2] * c[2] > 0.0,
            "triangle {i} normal points inward"
        );
    }
}

#[test]
fn chk_bem_1_a_pulsating_sphere_matches_the_closed_form() {
    let a = 0.1;
    let mesh = icosphere(a, 2).expect("icosphere");
    assert_eq!(mesh.triangles.len(), 320);
    let exact_area = 4.0 * std::f64::consts::PI * a * a;
    assert!((mesh.surface_area() - exact_area).abs() / exact_area < 0.02);

    let v_n = 1.0;
    let velocities = vec![v_n; mesh.triangles.len()];
    let kas = [0.5, 1.0, 2.0];
    let mut worst_db = 0.0_f64;
    for &ka in &kas {
        let freq = ka * C0 / (2.0 * std::f64::consts::PI * a);
        let pressures = solve_neumann(&mesh, freq, &velocities).expect("solve");
        let expected = pulsating_sphere_pressure(ka, v_n);
        let mut sum = num_complex::Complex64::new(0.0, 0.0);
        let mut weight = 0.0;
        for (p, area) in pressures.iter().zip(mesh.areas.iter()) {
            sum += *p * *area;
            weight += *area;
        }
        let mean = sum / weight;
        let err_db = 20.0 * (mean.norm() / expected.norm()).log10().abs();
        eprintln!(
            "ka={ka}: |p|_bem={:.4} |p|_exact={:.4} err={err_db:.3} dB phase_bem={:.1}° phase_ex={:.1}°",
            mean.norm(),
            expected.norm(),
            mean.arg().to_degrees(),
            expected.arg().to_degrees()
        );
        assert!(
            err_db < 0.5,
            "ka={ka}: surface |p| {err_db:.2} dB from closed form"
        );
        worst_db = worst_db.max(err_db);
        let phase_err = (mean.arg() - expected.arg()).abs();
        assert!(
            phase_err < 10.0_f64.to_radians(),
            "ka={ka}: phase error {} deg",
            phase_err.to_degrees()
        );
    }
    eprintln!("worst |p| error {worst_db:.3} dB on 320-face icosphere");
}
