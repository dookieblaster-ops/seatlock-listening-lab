//! Oscillating-sphere Neumann radiation (CHK-BEM-3): non-uniform q on the exterior CBIE.

use seatlock_bem::{icosphere, oscillating_sphere_pressure, solve_neumann_q};
use seatlock_core::{C0, RHO0};
use std::f64::consts::PI;

#[test]
fn the_oscillating_sphere_closed_form_has_the_right_low_ka_slope() {
    // Small ka: |h₁/h₁'| ~ ka/2, so |p| ~ (ρ c U ka / 2) |cos θ|.
    let ka = 0.05;
    let p = oscillating_sphere_pressure(ka, 1.0, 1.0);
    let expected = RHO0 * C0 * ka / 2.0;
    assert!(
        (p.norm() - expected).abs() / expected < 0.05,
        "|p|={} vs ~{}",
        p.norm(),
        expected
    );
}

#[test]
fn chk_bem_3_an_oscillating_sphere_matches_the_closed_form() {
    let a = 0.1;
    let mesh = icosphere(a, 2).expect("icosphere");
    assert_eq!(mesh.triangles.len(), 320);
    let u = 1.0;
    let kas = [0.5, 1.0, 2.0];
    for &ka in &kas {
        let freq = ka * C0 / (2.0 * PI * a);
        let omega = 2.0 * PI * freq;
        let mut q = Vec::with_capacity(mesh.triangles.len());
        let mut p_ex = Vec::with_capacity(mesh.triangles.len());
        for (c, n) in mesh.centroids.iter().zip(&mesh.normals) {
            let r = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
            let cos_theta = c[2] / r;
            // Outward normal velocity on the facet ≈ U (ẑ · n).
            let v_n = u * n[2];
            q.push(num_complex::Complex64::new(0.0, -omega * RHO0) * v_n);
            p_ex.push(oscillating_sphere_pressure(ka, cos_theta, u));
        }
        let p = solve_neumann_q(&mesh, freq, &q).expect("solve");
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..mesh.triangles.len() {
            num += (p[i] - p_ex[i]).norm() * mesh.areas[i];
            den += p_ex[i].norm().max(1e-12) * mesh.areas[i];
        }
        let rel = num / den;
        eprintln!("ka={ka}: area-weighted relative L1 error {rel:.4}");
        assert!(
            rel < 0.03,
            "ka={ka}: relative L1 error {rel:.3} (limit 0.03 for constant-element CBIE)"
        );
    }
}
