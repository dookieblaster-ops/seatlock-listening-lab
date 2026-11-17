//! Exterior field evaluation (CHK-BEM-4) and box mesh sanity.

use num_complex::Complex64;
use seatlock_bem::{
    box_aabb, field_pressure, icosphere, pulsating_sphere_field, pulsating_sphere_pressure,
    solve_neumann, transfer_from_velocity, NeumannSystem,
};
use seatlock_core::C0;

#[test]
fn the_pulsating_sphere_field_matches_the_surface_at_r_equals_a() {
    let surface = pulsating_sphere_pressure(1.0, 1.0);
    let field = pulsating_sphere_field(1.0, 0.1, 0.1, 1.0);
    assert!((field - surface).norm() < 1e-12);
}

#[test]
fn a_box_aabb_orients_normals_outward() {
    let mesh = box_aabb([0.2, 0.16, 0.24], 2).unwrap();
    assert_eq!(mesh.triangles.len(), 6 * 2 * 2 * 2);
    let center = [0.1, 0.08, 0.12];
    for (i, (n, c)) in mesh.normals.iter().zip(mesh.centroids.iter()).enumerate() {
        let from_center = [c[0] - center[0], c[1] - center[1], c[2] - center[2]];
        assert!(
            n[0] * from_center[0] + n[1] * from_center[1] + n[2] * from_center[2] > 0.0,
            "triangle {i} normal points inward"
        );
    }
}

#[test]
fn chk_bem_4_pulsating_sphere_field_matches_the_closed_form() {
    let a = 0.1;
    let mesh = icosphere(a, 2).expect("icosphere");
    let v_n = 1.0;
    let velocities = vec![v_n; mesh.triangles.len()];
    let r = 1.0;
    let point = [0.0, -r, 0.0];
    let kas = [0.5, 1.0, 2.0];
    let mut worst_db = 0.0_f64;
    for &ka in &kas {
        let freq = ka * C0 / (2.0 * std::f64::consts::PI * a);
        let system = NeumannSystem::assemble(&mesh, freq).expect("assemble");
        let complex_v: Vec<_> = velocities.iter().map(|&v| Complex64::new(v, 0.0)).collect();
        let p_surf = system.solve_velocity(&complex_v).expect("solve");
        let omega = 2.0 * std::f64::consts::PI * freq;
        let q: Vec<_> = complex_v
            .iter()
            .map(|&v| Complex64::new(0.0, -omega * seatlock_core::RHO0) * v)
            .collect();
        let bem = field_pressure(&mesh, system.k, &p_surf, &q, point).expect("field");
        let exact = pulsating_sphere_field(ka, a, r, v_n);
        let err_db = 20.0 * (bem.norm() / exact.norm()).log10().abs();
        let phase_deg = (bem / exact).arg().to_degrees().abs();
        eprintln!(
            "ka={ka}: |p|_bem={:.5} |p|_exact={:.5} err={err_db:.3} dB phase={phase_deg:.2}°",
            bem.norm(),
            exact.norm()
        );
        assert!(
            err_db < 0.5,
            "ka={ka}: field |p| {err_db:.2} dB from closed form"
        );
        assert!(
            phase_deg < 10.0,
            "ka={ka}: field phase {phase_deg:.1}° from closed form"
        );
        worst_db = worst_db.max(err_db);
    }
    eprintln!("worst field |p| error {worst_db:.3} dB at 1 m on 320-face icosphere");
}

#[test]
fn transfer_from_unit_volume_velocity_matches_scaled_field() {
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    let area = mesh.surface_area();
    let freq = 0.5 * C0 / (2.0 * std::f64::consts::PI * a);
    let system = NeumannSystem::assemble(&mesh, freq).unwrap();
    // Unit volume velocity: v_n = 1/area everywhere.
    let v_n: Vec<_> = mesh
        .areas
        .iter()
        .map(|_| Complex64::new(1.0 / area, 0.0))
        .collect();
    let point = [0.0, -1.0, 0.0];
    let transfer = transfer_from_velocity(&system, &mesh, &v_n, point).unwrap();
    let v_one = vec![1.0; mesh.triangles.len()];
    let p_surf = solve_neumann(&mesh, freq, &v_one).unwrap();
    let omega = 2.0 * std::f64::consts::PI * freq;
    let q: Vec<_> = v_one
        .iter()
        .map(|&v| Complex64::new(0.0, -omega * seatlock_core::RHO0) * v)
        .collect();
    let full = field_pressure(&mesh, system.k, &p_surf, &q, point).unwrap();
    // full is for U = area * 1; transfer is per unit U.
    let expected = full / area;
    let err = (transfer - expected).norm() / expected.norm();
    assert!(err < 1e-9, "relative error {err}");
}
