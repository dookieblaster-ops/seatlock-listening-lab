//! Layered and direction-dependent panels (README section 5.9): CHK-PLATE-2 and the laminate's
//! stiffness against hand calculations.

use seatlock_acoustics::laminate::{laminate, Layer, Stiffness, ASSUMED_LOSS_FACTOR};
use seatlock_acoustics::plate::{Edges, Plate, PlateModes};
use serde_json::json;

fn layer(record: serde_json::Value, thickness_m: f64, turned: bool) -> Layer {
    Layer::from_record("test", &record, thickness_m, turned).unwrap()
}

/// The balsa elastic ratios of the Wood Handbook (Table 5-1, 5-2) on a 3.74 GPa grain
/// stiffness: a strongly direction-dependent plate.
fn orthotropic(edges: Edges) -> Plate {
    let e_l = 3.74e9;
    let wood = layer(
        json!({ "density_kg_m3": 179.0, "youngs_u_pa": e_l, "youngs_v_pa": 0.015 * e_l,
                "poisson_uv": 0.488, "shear_pa": 0.037 * e_l, "loss_factor": 0.01 }),
        0.004,
        false,
    );
    let mut stiffness = laminate(&[wood]).unwrap().stiffness;
    stiffness.shear_per_width = f64::INFINITY;
    Plate {
        width_m: 0.40,
        height_m: 0.30,
        layup: stiffness,
        edges,
    }
}

#[test]
fn chk_plate_2_orthotropic_supported_resonances_match_the_closed_form() {
    // Lekhnitskii: omega^2 = (D11 p^4 + 2(D12 + 2 D66) p^2 q^2 + D22 q^4) / m for a simply
    // supported orthotropic plate, p = i*pi/a, q = j*pi/b.
    let plate = orthotropic(Edges::Supported);
    let modes = PlateModes::solve(plate, 24).unwrap();
    let mut exact = Vec::new();
    for i in 1..60 {
        for j in 1..60 {
            let f = plate.supported_mode_hz(i, j);
            // Modes near the limit can land on either side of it.
            if f <= 0.95 * modes.resolved_up_to_hz {
                exact.push(f);
            }
        }
    }
    exact.sort_by(f64::total_cmp);
    let got = modes.frequencies_hz();
    assert!(
        exact.len() >= 25,
        "only {} resonances resolved",
        exact.len()
    );
    assert!(got.len() >= exact.len());
    let worst = got
        .iter()
        .zip(&exact)
        .map(|(g, e)| (g / e - 1.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.01, "worst {worst}");
    println!(
        "orthotropic: {} resonances up to {:.0} Hz, worst {:.4} %",
        got.len(),
        modes.resolved_up_to_hz,
        worst * 100.0
    );
}

#[test]
fn one_isotropic_layer_gives_the_plate_formulas() {
    let (e, nu, h) = (2.68e9, 0.35, 0.003);
    let one = layer(
        json!({ "density_kg_m3": 1240.0, "youngs_pa": e, "poisson": nu, "loss_factor": 0.03 }),
        h,
        false,
    );
    let stack = laminate(&[one]).unwrap();
    let d = e * h.powi(3) / (12.0 * (1.0 - nu * nu));
    let s = stack.stiffness;
    for (got, want) in [
        (s.d11, d),
        (s.d22, d),
        (s.d12, nu * d),
        (s.d66, (1.0 - nu) * d / 2.0),
        (s.mass_per_area, 1240.0 * h),
        (s.shear_per_width, 5.0 / 6.0 * e / (2.0 * (1.0 + nu)) * h),
    ] {
        assert!((got / want - 1.0).abs() < 1e-12, "{got} against {want}");
    }
    assert!((stack.loss_factor - 0.03).abs() < 1e-12);
    assert!(stack.notes.is_empty());
}

#[test]
fn a_sandwich_matches_the_hand_calculation() {
    // PLA skins 0.4 mm over a 5 mm foam core: D = Q_f*(h^3 - t_c^3)/12 + Q_c*t_c^3/12,
    // S = d^2 / (t_c/G_c + 2*t_f/G_f) with d = h - t_f.
    let skin = json!({ "density_kg_m3": 1240.0, "youngs_pa": 2.68e9, "poisson": 0.35,
                       "loss_factor": 0.02 });
    let core = json!({ "density_kg_m3": 24.0, "youngs_pa": 11e6, "poisson": 0.3,
                       "shear_pa": 2.275e6, "loss_factor": 0.05 });
    let (tf, tc) = (0.0004, 0.005);
    let stack = laminate(&[
        layer(skin.clone(), tf, false),
        layer(core, tc, false),
        layer(skin, tf, false),
    ])
    .unwrap();
    let h = tc + 2.0 * tf;
    let qf = 2.68e9 / (1.0 - 0.35 * 0.35);
    let qc = 11e6 / (1.0 - 0.3 * 0.3);
    let d = qf * (h.powi(3) - tc.powi(3)) / 12.0 + qc * tc.powi(3) / 12.0;
    let gf = 2.68e9 / (2.0 * 1.35);
    let shear = (h - tf).powi(2) / (tc / 2.275e6 + 2.0 * tf / gf);
    let s = stack.stiffness;
    assert!((s.d11 / d - 1.0).abs() < 1e-12, "{} against {d}", s.d11);
    assert!((s.d22 / d - 1.0).abs() < 1e-12);
    assert!((s.shear_per_width / shear - 1.0).abs() < 1e-12);
    assert!((s.mass_per_area - (2.0 * tf * 1240.0 + tc * 24.0)).abs() < 1e-12);
    // The core's loss weighs by its small share of the stiffness.
    let core_share = qc * tc.powi(3) / 12.0 / d;
    let want = 0.02 * (1.0 - core_share) + 0.05 * core_share;
    assert!((stack.loss_factor - want).abs() < 1e-12);
}

#[test]
fn a_quarter_turn_swaps_the_directions_and_cross_plies_balance() {
    let veneer = json!({ "density_kg_m3": 694.0, "youngs_u_pa": 15.29e9,
                         "youngs_v_pa": 0.765e9, "poisson_uv": 0.451, "shear_pa": 1.04e9,
                         "loss_factor": 0.01 });
    let along = laminate(&[layer(veneer.clone(), 0.001, false)])
        .unwrap()
        .stiffness;
    let across = laminate(&[layer(veneer.clone(), 0.001, true)])
        .unwrap()
        .stiffness;
    assert!((along.d11 / across.d22 - 1.0).abs() < 1e-12);
    assert!((along.d22 / across.d11 - 1.0).abs() < 1e-12);
    assert!((along.d12 / across.d12 - 1.0).abs() < 1e-12);
    // Three plies 0/90/0: the outer plies carry u, the middle one v. Hand calculation:
    let t: f64 = 0.001;
    let nu_vu = 0.451 * 0.765 / 15.29;
    let den = 1.0 - 0.451 * nu_vu;
    let (q11, q22) = (15.29e9 / den, 0.765e9 / den);
    let outer = ((1.5 * t).powi(3) - (0.5 * t).powi(3)) / 3.0 * 2.0;
    let middle = (0.5 * t).powi(3) * 2.0 / 3.0;
    let ply = laminate(&[
        layer(veneer.clone(), t, false),
        layer(veneer.clone(), t, true),
        layer(veneer, t, false),
    ])
    .unwrap()
    .stiffness;
    assert!((ply.d11 / (q11 * outer + q22 * middle) - 1.0).abs() < 1e-12);
    assert!((ply.d22 / (q22 * outer + q11 * middle) - 1.0).abs() < 1e-12);
}

#[test]
fn shear_through_a_soft_core_lowers_the_resonances_as_a_mindlin_plate() {
    // A simply supported sandwich: the Rayleigh-Ritz modes with the shear estimate against
    // the closed form, and the shear lowering the high modes far more than the low ones.
    let skin = json!({ "density_kg_m3": 1240.0, "youngs_pa": 2.68e9, "poisson": 0.35,
                       "loss_factor": 0.02 });
    let core = json!({ "density_kg_m3": 24.0, "youngs_pa": 11e6, "poisson": 0.3,
                       "shear_pa": 2.275e6, "loss_factor": 0.05 });
    let stack = laminate(&[
        layer(skin.clone(), 0.0004, false),
        layer(core, 0.005, false),
        layer(skin, 0.0004, false),
    ])
    .unwrap();
    let plate = Plate {
        width_m: 0.40,
        height_m: 0.30,
        layup: stack.stiffness,
        edges: Edges::Supported,
    };
    let stiff = Plate {
        layup: Stiffness {
            shear_per_width: f64::INFINITY,
            ..stack.stiffness
        },
        ..plate
    };
    let modes = PlateModes::solve(plate, 24).unwrap();
    let got = modes.frequencies_hz();
    let mut exact: Vec<f64> = (1..40)
        .flat_map(|i| (1..40).map(move |j| (i, j)))
        .map(|(i, j)| plate.supported_mode_hz(i, j))
        .filter(|f| *f <= 0.95 * modes.resolved_up_to_hz)
        .collect();
    exact.sort_by(f64::total_cmp);
    assert!(got.len() >= exact.len());
    for (g, e) in got.iter().zip(&exact) {
        assert!((g / e - 1.0).abs() < 0.01, "{g} against {e}");
    }
    let low = plate.supported_mode_hz(1, 1) / stiff.supported_mode_hz(1, 1);
    let high = plate.supported_mode_hz(8, 8) / stiff.supported_mode_hz(8, 8);
    assert!(low < 1.0 && high < low, "low {low}, high {high}");
    println!("sandwich: first mode x{low:.4}, mode (8,8) x{high:.4} from shear");
}

#[test]
fn missing_figures_are_assumed_and_said() {
    let stack = laminate(&[layer(
        json!({ "name": "Foam", "density_kg_m3": 24.0, "youngs_pa": 11e6 }),
        0.006,
        false,
    )])
    .unwrap();
    assert!((stack.loss_factor - ASSUMED_LOSS_FACTOR).abs() < 1e-12);
    assert_eq!(stack.notes.len(), 2, "{:?}", stack.notes);
    assert!(stack.notes.iter().any(|n| n.contains("Poisson")));
    assert!(stack.notes.iter().any(|n| n.contains("loss factor")));
}

#[test]
fn a_core_alone_cannot_bend() {
    let core = json!({ "density_kg_m3": 80.0, "core_only": true, "transverse_shear_pa": 9e6 });
    assert!(laminate(&[layer(core, 0.005, false)]).is_err());
}
