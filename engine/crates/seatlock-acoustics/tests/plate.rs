//! CHK-PLATE-1 and the other edge conditions of the panel's plate model (README section 5.9).

use seatlock_acoustics::plate::{Edges, Plate, PlateModes};
use std::f64::consts::PI;

/// `test-panel-a` of README section 13.
fn test_panel(edges: Edges) -> Plate {
    Plate::isotropic(0.40, 0.30, 0.003, 3.5e9, 1240.0, 0.35, edges)
}

fn square(edges: Edges) -> Plate {
    Plate::isotropic(0.3, 0.3, 0.003, 3.5e9, 1240.0, 0.3, edges)
}

/// Frequency parameter `omega * a^2 * sqrt(m / D)` of each resonance of a square plate.
fn parameters(modes: &PlateModes) -> Vec<f64> {
    let plate = modes.plate();
    let scale = plate.width_m.powi(2) * (plate.mass_per_area() / plate.stiffness()).sqrt();
    modes
        .frequencies_hz()
        .iter()
        .map(|freq| 2.0 * PI * freq * scale)
        .collect()
}

#[test]
fn chk_plate_1_simply_supported_resonances() {
    let modes = PlateModes::solve(test_panel(Edges::Supported), 24).unwrap();
    let got = modes.frequencies_hz();
    for (index, expected) in [42.36, 88.10, 123.68, 164.35, 169.43].iter().enumerate() {
        assert!(
            (got[index] / expected - 1.0).abs() < 0.01,
            "mode {index}: {} Hz against {expected} Hz",
            got[index]
        );
    }
}

#[test]
fn every_resolved_resonance_of_the_supported_panel_matches_the_closed_form() {
    let plate = test_panel(Edges::Supported);
    let mut exact: Vec<f64> = Vec::new();
    for i in 1..80 {
        for j in 1..80 {
            exact.push(plate.supported_mode_hz(i, j));
        }
    }
    exact.sort_by(f64::total_cmp);
    for (degree, at_least) in [(24, 80), (32, 150)] {
        let started = std::time::Instant::now();
        let modes = PlateModes::solve(plate, degree).unwrap();
        let got = modes.frequencies_hz();
        let mut worst = 0.0_f64;
        for (index, freq) in got.iter().enumerate() {
            worst = worst.max((freq / exact[index] - 1.0).abs());
        }
        println!(
            "degree {degree}: {} resonances up to {:.0} Hz, worst error {:.3} %, {:?}",
            got.len(),
            modes.resolved_up_to_hz,
            100.0 * worst,
            started.elapsed()
        );
        assert!(got.len() >= at_least, "{}", got.len());
        // Every resonance the solver keeps is within 1 % of the closed form, in order, so
        // none is missing or doubled below the limit it states.
        assert!(worst < 0.01, "{worst}");
        let below = exact
            .iter()
            .filter(|freq| **freq <= 0.99 * modes.resolved_up_to_hz)
            .count();
        assert!(got.len() >= below, "{} of {below}", got.len());
    }
}

#[test]
fn a_free_square_plate_matches_the_published_frequency_parameters() {
    // Leissa, "Vibration of Plates" (NASA SP-160), completely free square plate, nu = 0.3:
    // 13.47, 19.60, 24.27, 34.80 (twice), 61.09 (twice). Three rigid-body modes come first.
    let modes = PlateModes::solve(square(Edges::Free), 20).unwrap();
    let got = parameters(&modes);
    for value in &got[..3] {
        assert!(value.abs() < 1e-3, "{value}");
    }
    for (index, expected) in [13.47, 19.60, 24.27, 34.80, 34.80, 61.09, 61.09]
        .iter()
        .enumerate()
    {
        let value = got[index + 3];
        println!("free mode {index}: {value:.3} against {expected}");
        assert!(
            (value / expected - 1.0).abs() < 0.005,
            "{value} against {expected}"
        );
    }
}

#[test]
fn a_clamped_square_plate_matches_the_published_frequency_parameters() {
    // Leissa, same book, fully clamped square plate: 35.99, 73.41 (twice), 108.27.
    let modes = PlateModes::solve(square(Edges::Clamped), 20).unwrap();
    let got = parameters(&modes);
    for (index, expected) in [35.99, 73.41, 73.41, 108.27].iter().enumerate() {
        println!("clamped mode {index}: {:.3} against {expected}", got[index]);
        assert!(
            (got[index] / expected - 1.0).abs() < 0.005,
            "{}",
            got[index]
        );
    }
}

#[test]
fn mode_shapes_have_unit_modal_mass_and_do_not_mix() {
    let plate = test_panel(Edges::Free);
    let modes = PlateModes::solve(plate, 16).unwrap();
    // Midpoint rule over the panel: integral of m * w_a * w_b is 1 for a = b and 0 otherwise.
    let (nu, nv) = (80usize, 60usize);
    let cell = plate.width_m / nu as f64 * plate.height_m / nv as f64;
    let pick = [0usize, 3, 5, 9];
    let mut gram = vec![vec![0.0; pick.len()]; pick.len()];
    for iu in 0..nu {
        for iv in 0..nv {
            let shapes = modes.shapes_at(
                (iu as f64 + 0.5) * plate.width_m / nu as f64,
                (iv as f64 + 0.5) * plate.height_m / nv as f64,
            );
            for (a, &ma) in pick.iter().enumerate() {
                for (b, &mb) in pick.iter().enumerate() {
                    gram[a][b] += plate.mass_per_area() * cell * shapes[ma] * shapes[mb];
                }
            }
        }
    }
    for (a, row) in gram.iter().enumerate() {
        for (b, value) in row.iter().enumerate() {
            let expected = if a == b { 1.0 } else { 0.0 };
            assert!((value - expected).abs() < 0.01, "{a} {b}: {value}");
        }
    }
}

#[test]
fn the_exciter_sees_the_mobility_of_a_large_plate_on_average() {
    // The mean-value rule: averaged over a band with many resonances, a finite plate's
    // drive-point mobility is that of an endless plate, 1 / (8 * sqrt(D * m)). Seen from the
    // exciter's terminals, with the magnet nearly still and the suspension soft, that is a
    // resistance of bl^2 times it on top of the coil's own.
    use num_complex::Complex64;
    use seatlock_core::registry::Registry;
    use serde_json::json;
    let mut registry = Registry::new();
    seatlock_acoustics::register(&mut registry);
    let fields = json!({
        "exciter_id": "test-exciter-a", "mount": "m1", "backing": "sealed", "cavity_depth_m": 0.02,
        "panel": { "width_m": 0.40, "height_m": 0.30, "thickness_m": 0.003,
                   "material": "test-panel-material-a", "edges": "free" },
        "exciter_at_m": [0.17, 0.11]
    });
    let panel = registry
        .create_radiator("dml_panel", fields.as_object().unwrap())
        .unwrap();
    assert!(!panel.loads_box());
    let freqs: Vec<f64> = (0..400).map(|i| 300.0 + 700.0 * i as f64 / 399.0).collect();
    let eval = panel.evaluate(&freqs, &[]);
    let plate = test_panel(Edges::Free);
    let endless = 1.0 / (8.0 * (plate.stiffness() * plate.mass_per_area()).sqrt());
    let mean: f64 = eval
        .impedance
        .iter()
        .map(|z: &Complex64| z.re - 4.0)
        .sum::<f64>()
        / freqs.len() as f64;
    let expected = 3.0 * 3.0 * endless;
    println!("added resistance {mean:.4} ohm against {expected:.4} ohm");
    assert!(
        (mean / expected - 1.0).abs() < 0.25,
        "{mean} against {expected}"
    );
    // The panel's sources: a grid that covers the panel, each with its area.
    let elements = panel.surface(&freqs[..2]).unwrap();
    let area: f64 = elements.iter().map(|element| element.area_m2).sum();
    assert!((area - 0.12).abs() < 1e-9);
    assert!(elements.iter().all(|element| element.at_m[0].abs() < 0.2
        && element.at_m[1].abs() < 0.15
        && element.volume_velocity.len() == 2));
    // Their volume velocities add up to the panel's net volume velocity.
    let net: Complex64 = elements
        .iter()
        .map(|element| element.volume_velocity[0])
        .sum();
    let reported = panel.evaluate(&freqs[..1], &[]).volume_velocity[0];
    assert!((net - reported).norm() < 1e-9 * reported.norm().max(1e-12) + 1e-15);
}
