//! Rigid sphere in a plane wave (CHK-BEM-2): surface pressures toward CHK-RAD-3.
//!
//! Low `ka` (below about 0.5) matches the closed form within the lit/side CHK-RAD-3
//! budget. Higher `ka` still needs Burton–Miller / a formulation pass before the full
//! CHK-RAD-3 band (up to `ka` 6) can be claimed for BEM.

use seatlock_bem::{icosphere, rigid_plane_wave_surface, rigid_sphere_surface_pressure};
use seatlock_core::C0;
use std::f64::consts::PI;

#[test]
fn the_rigid_sphere_series_has_the_right_limits() {
    assert!((rigid_sphere_surface_pressure(0.01, 0.0).norm() - 1.0).abs() < 1e-3);
    assert!((rigid_sphere_surface_pressure(40.0, 0.0).norm() - 2.0).abs() < 0.05);
}

#[test]
fn chk_bem_2_a_rigid_sphere_matches_the_closed_form_at_low_ka() {
    // Same geometry and wave as CHK-RAD-3: a = 0.1 m, wave toward −y (lit at y = −a).
    let a = 0.1;
    let mesh = icosphere(a, 2).expect("icosphere");
    assert_eq!(mesh.triangles.len(), 320);
    let toward = [0.0, -1.0, 0.0];
    let kas = [0.25, 0.5];
    let poles = [
        ([0.0, -a, 0.0], 0.0),     // lit
        ([a, 0.0, 0.0], PI / 2.0), // side
        ([0.0, a, 0.0], PI),       // shadow
    ];
    let mut worst = [0.0_f64; 3];
    for &ka in &kas {
        let freq = ka * C0 / (2.0 * PI * a);
        let pressures = rigid_plane_wave_surface(&mesh, freq, toward).expect("solve");
        for (slot, (pole, theta)) in poles.iter().enumerate() {
            let mut sum = num_complex::Complex64::new(0.0, 0.0);
            let mut weight = 0.0;
            for (i, (p, area)) in pressures.iter().zip(mesh.areas.iter()).enumerate() {
                let c = mesh.centroids[i];
                let d = ((c[0] - pole[0]).powi(2)
                    + (c[1] - pole[1]).powi(2)
                    + (c[2] - pole[2]).powi(2))
                .sqrt();
                if d < 0.35 * a {
                    sum += *p * *area;
                    weight += *area;
                }
            }
            assert!(weight > 0.0, "ka={ka} pole {slot}: no faces near the pole");
            let mean = sum / weight;
            let expected = rigid_sphere_surface_pressure(ka, *theta);
            let got_db = 20.0 * mean.norm().log10();
            let expected_db = 20.0 * expected.norm().log10();
            let err = (got_db - expected_db).abs();
            eprintln!(
                "ka={ka} point {slot}: bem {got_db:+.2} dB, closed {expected_db:+.2} dB, err {err:.3} dB"
            );
            worst[slot] = worst[slot].max(err);
            assert!(
                err < 0.75,
                "ka={ka} point {slot}: {err:.2} dB from closed form (0.75 dB lit/side budget)"
            );
        }
    }
    eprintln!(
        "worst |p| errors lit/side/shadow at ka≤0.5: {:.3} / {:.3} / {:.3} dB",
        worst[0], worst[1], worst[2]
    );
}
