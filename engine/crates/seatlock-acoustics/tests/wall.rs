//! Cabinet wall vibration (README section 5.11): CHK-WALL-1 and CHK-WALL-2 and the wall
//! stack against hand calculations.

use num_complex::Complex64;
use seatlock_acoustics::laminate::{laminate, Layer};
use seatlock_acoustics::wall::{WallLayer, WallPanel, WallStack};
use serde_json::json;
use std::f64::consts::PI;

fn pla(thickness_m: f64, loss_factor: f64) -> WallLayer {
    WallLayer {
        name: "PLA".into(),
        thickness_m,
        youngs_pa: 2.68e9,
        poisson: 0.3,
        density_kg_m3: 1240.0,
        loss_factor,
    }
}

/// Navier's series for a simply supported plate under uniform pressure: the outward volume
/// velocity per pascal, from every odd-odd mode below `limit_hz` (or all, up to `terms`).
#[allow(clippy::too_many_arguments)]
fn navier(
    a: f64,
    b: f64,
    d: f64,
    m: f64,
    eta: f64,
    freq_hz: f64,
    limit_hz: f64,
    terms: usize,
) -> Complex64 {
    let omega = 2.0 * PI * freq_hz;
    let mut sum = Complex64::new(0.0, 0.0);
    for i in (1..terms).step_by(2) {
        for j in (1..terms).step_by(2) {
            let k2 = (i as f64 * PI / a).powi(2) + (j as f64 * PI / b).powi(2);
            let lambda = d / m * k2 * k2;
            if lambda.sqrt() / (2.0 * PI) > limit_hz {
                continue;
            }
            let s2 = 64.0 * a * b / (m * PI.powi(4) * (i * i * j * j) as f64);
            sum += s2 / (Complex64::new(lambda, lambda * eta) - omega * omega);
        }
    }
    Complex64::new(0.0, omega) * sum
}

#[test]
fn chk_wall_1_a_supported_wall_under_uniform_pressure_matches_navier() {
    let (a, b, h, eta) = (0.152, 0.242, 0.008, 0.02);
    let stack = WallStack::from_layers(&[pla(h, eta)]).unwrap();
    let d = 2.68e9 * h.powi(3) / (12.0 * (1.0 - 0.09));
    let m = 1240.0 * h;
    assert!((stack.reference() / d - 1.0).abs() < 1e-12);
    let panel = WallPanel::new(a, b, &stack).unwrap();
    let limit = panel.resolved_up_to_hz;
    // Every mode that the pressure drives, against the closed form.
    let mut driven: Vec<(f64, f64)> = panel
        .lambdas
        .iter()
        .zip(&panel.integrals)
        .filter(|(_, s)| {
            s.abs() > 1e-6 * panel.integrals.iter().fold(0.0f64, |x, y| x.max(y.abs()))
        })
        .map(|(l, s)| (l.re.sqrt() / (2.0 * PI), s * s))
        .collect();
    driven.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut exact = Vec::new();
    for i in (1..99).step_by(2) {
        for j in (1..99).step_by(2) {
            let k2 = (i as f64 * PI / a).powi(2) + (j as f64 * PI / b).powi(2);
            let f = (d / m).sqrt() * k2 / (2.0 * PI);
            // Modes near the resolved limit can land on either side of it.
            if f <= 0.95 * limit {
                exact.push((f, 64.0 * a * b / (m * PI.powi(4) * (i * i * j * j) as f64)));
            }
        }
    }
    exact.sort_by(|x, y| x.0.total_cmp(&y.0));
    assert!(
        exact.len() >= 6,
        "only {} driven modes resolved",
        exact.len()
    );
    assert!(
        driven.len() >= exact.len(),
        "{} driven modes, {} expected below {:.0} Hz: {:?}",
        driven.len(),
        exact.len(),
        0.95 * limit,
        driven.iter().map(|x| x.0 as i64).collect::<Vec<_>>()
    );
    for ((f, s2), (fe, s2e)) in driven.iter().zip(&exact) {
        // The basis resolves modes within 1 % up to its limit (README 5.9); the lower half
        // to within 0.1 %. A shape near the limit is less exact than its frequency: measured
        // 2.8 % in its integral (a mode with nine half-waves one way), against 0.2 % below.
        let (tolerance, shape) = if *fe < 0.5 * limit {
            (1e-3, 5e-3)
        } else {
            (1e-2, 5e-2)
        };
        assert!((f / fe - 1.0).abs() < tolerance, "{f} Hz against {fe} Hz");
        assert!(
            (s2 / s2e - 1.0).abs() < shape,
            "s^2 {s2} against {s2e} at {fe} Hz"
        );
    }
    // The loss enters every mode alike.
    for lambda in &panel.lambdas {
        assert!((lambda.im / lambda.re - eta).abs() < 1e-12);
    }
    // The wall's response: far below, at, and between its resonances.
    let f11 = exact[0].0;
    let mut checked = vec![5.0, 0.5 * f11, f11, exact[1].0, exact[2].0];
    checked.push(0.5 * (exact[0].0 + exact[1].0));
    for f in checked {
        let got = panel.admittance(f);
        let want = navier(a, b, d, m, eta, f, limit, 199);
        assert!(
            (got - want).norm() < 0.01 * want.norm(),
            "{f:.1} Hz: {got} against {want}"
        );
    }
    // Static compliance against the whole series (the modes left out add little).
    let static_got = panel.admittance(1.0) / Complex64::new(0.0, 2.0 * PI);
    let static_all =
        navier(a, b, d, m, 0.0, 1.0, f64::INFINITY, 401) / Complex64::new(0.0, 2.0 * PI);
    assert!(
        (static_got.re / static_all.re - 1.0).abs() < 0.01,
        "{} against {}",
        static_got.re,
        static_all.re
    );
    // The elements add up to the wall.
    for f in [10.0, f11, 3.0 * f11] {
        let sum: Complex64 = panel.element_admittances(f).iter().sum();
        assert!((sum - panel.admittance(f)).norm() < 1e-9 * panel.admittance(f).norm());
    }
    println!(
        "CHK-WALL-1: {} driven modes up to {limit:.0} Hz, first {f11:.1} Hz",
        exact.len()
    );
}

#[test]
fn chk_wall_2_a_soft_core_damps_as_ross_kerwin_ungar_predict() {
    // Two lossless PLA skins on a soft lossy core.
    let (tf, tc, ec, beta) = (0.0016, 0.0048, 4.1e6, 0.5);
    let core = WallLayer {
        name: "core".into(),
        thickness_m: tc,
        youngs_pa: ec,
        poisson: 0.3,
        density_kg_m3: 813.0,
        loss_factor: beta,
    };
    let stack = WallStack::from_layers(&[pla(tf, 0.0), core, pla(tf, 0.0)]).unwrap();
    let qf = 2.68e9 / (1.0 - 0.09);
    let d = tf + tc;
    let d_local = 2.0 * qf * tf.powi(3) / 12.0;
    let d_sandwich = qf * tf * d * d / 2.0;
    let g_core = ec / (2.0 * 1.3);
    let shear = g_core * d * d / tc;
    let y = d_sandwich / d_local;
    // The core's own bending, which the published formula leaves out.
    let d_core = ec / 0.91 * tc.powi(3) / 12.0;
    let mut peak: f64 = 0.0;
    for k in [5.0, 10.0, 20.0, 40.0, 80.0, 160.0, 320.0] {
        let k2: f64 = k * k;
        let got = stack.stiffness(k2);
        let eta = got.im / got.re;
        // Assembled by hand: the stack must put the layers where they are.
        let lossy = Complex64::new(1.0, beta);
        // Shear compliance of the core and, far smaller, of the two skins.
        let skins = 2.0 * tf / (2.68e9 / 2.6);
        let shear_complex = d * d / (tc / (lossy * g_core) + skins);
        let g_complex = shear_complex / (d_sandwich * k2);
        let hand = d_local + d_core * lossy + d_sandwich * g_complex / (1.0 + g_complex);
        assert!(
            (got - hand).norm() < 1e-9 * hand.norm(),
            "k {k}: {got} against {hand}"
        );
        // Mead-Markus / Ross-Kerwin-Ungar, which differs only by the core's own bending loss.
        let g = shear / (d_sandwich * k2);
        let rku = beta * y * g / (1.0 + (2.0 + y) * g + (1.0 + y) * (1.0 + beta * beta) * g * g);
        let bound = beta * d_core / d_local * 1.05 + 1e-4;
        assert!((eta - rku).abs() < bound, "k {k}: {eta} against {rku}");
        peak = peak.max(rku);
    }
    let g_best = 1.0 / ((1.0 + y) * (1.0 + beta * beta)).sqrt();
    let eta_best = beta * y / (2.0 + y + 2.0 / g_best);
    assert!(
        peak <= eta_best * 1.03 && peak > 0.5 * eta_best,
        "{peak} against {eta_best}"
    );
    println!("CHK-WALL-2: Y {y:.1}, best loss factor {eta_best:.3}, largest found {peak:.3}");
}

#[test]
fn a_stiff_core_gives_the_laminates_stiffness_and_a_loose_fill_only_mass() {
    let gypsum = WallLayer {
        name: "gypsum".into(),
        thickness_m: 0.0048,
        youngs_pa: 10.4e9,
        poisson: 0.3,
        density_kg_m3: 1442.0,
        loss_factor: 0.0,
    };
    let stack =
        WallStack::from_layers(&[pla(0.0016, 0.0), gypsum.clone(), pla(0.0016, 0.0)]).unwrap();
    let record = |e: f64, rho: f64| json!({ "youngs_pa": e, "density_kg_m3": rho, "poisson": 0.3, "loss_factor": 0.0 });
    let clt = laminate(&[
        Layer::from_record("pla", &record(2.68e9, 1240.0), 0.0016, false).unwrap(),
        Layer::from_record("gypsum", &record(10.4e9, 1442.0), 0.0048, false).unwrap(),
        Layer::from_record("pla", &record(2.68e9, 1240.0), 0.0016, false).unwrap(),
    ])
    .unwrap();
    assert!((stack.reference() / clt.stiffness.d11 - 1.0).abs() < 1e-12);
    assert!((stack.mass_per_area / clt.stiffness.mass_per_area - 1.0).abs() < 1e-12);
    // A stiff core hardly shears at a wall's wavelengths.
    assert!((stack.stiffness(40.0f64.powi(2)).re / stack.reference() - 1.0).abs() < 0.01);

    let sand = WallLayer {
        name: "sand".into(),
        thickness_m: 0.0048,
        youngs_pa: 0.0,
        poisson: 0.0,
        density_kg_m3: 1602.0,
        loss_factor: 0.09,
    };
    let stack = WallStack::from_layers(&[pla(0.0016, 0.02), sand, pla(0.0016, 0.02)]).unwrap();
    let skin = Complex64::new(2.68e9, 2.68e9 * 0.02) / 0.91 * 0.0016f64.powi(3) / 12.0;
    for k in [0.0, 30.0, 300.0] {
        assert!((stack.stiffness(k * k) - 2.0 * skin).norm() < 1e-12 * skin.norm());
    }
    let mass = 2.0 * 1240.0 * 0.0016 + 1602.0 * 0.0048;
    assert!((stack.mass_per_area / mass - 1.0).abs() < 1e-12);
    assert!((stack.fill_loss - 0.09 * 1602.0 * 0.0048 / mass).abs() < 1e-12);
}

#[test]
fn bessel_j1_holds_at_large_arguments() {
    use seatlock_acoustics::bessel_j1;
    // Where the series is still exact, the two forms agree.
    let series = |x: f64| {
        let (mut sum, mut term) = (x / 2.0, x / 2.0);
        for m in 1..80 {
            term *= -(x * x) / (4.0 * m as f64 * (m as f64 + 1.0));
            sum += term;
        }
        sum
    };
    for x in [12.0, 13.5, 15.0] {
        assert!((bessel_j1(x) - series(x)).abs() < 1e-7, "{x}");
    }
    // Large arguments: bounded by the asymptotic envelope sqrt(2/(pi x)).
    for x in [20.0, 50.0, 100.0, 300.0] {
        assert!(bessel_j1(x).abs() <= (2.0 / (PI * x)).sqrt() * 1.01, "{x}");
    }
}
