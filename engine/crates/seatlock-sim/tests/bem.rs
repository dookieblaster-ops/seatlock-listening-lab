//! The `bem` radiation model inside `simulate` (README section 5.6), including mouth
//! self-impedances joined to the waveguide network (CHK-BEM-8).

use seatlock_acoustics::{radiation_impedance, with_fixed_radiation_loads, AcousticNode};
use seatlock_core::constants::C0;
use seatlock_core::registry::{RadiationScene, SourcePatch};
use seatlock_sim::bem::{self, NETWORK_NOTE};
use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::{json, Value};
use std::f64::consts::PI;

fn load_example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(value: &Value, radiation: &str, freqs: &[f64]) -> SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value.clone()).unwrap();
    simulate(
        &design,
        &SimulateRequest {
            radiation: radiation.into(),
            frequencies_hz: Some(freqs.to_vec()),
            bem_subdivisions: Some(4),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

fn run_waveguide(value: &Value, radiation: &str, freqs: &[f64]) -> SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value.clone()).unwrap();
    simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: radiation.into(),
            frequencies_hz: Some(freqs.to_vec()),
            bem_subdivisions: Some(4),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

#[test]
fn bem_is_registered_and_labels_the_outer_shape_mesh() {
    let freqs = [50.0, 100.0, 200.0];
    let result = run(&load_example("sealed-test"), "bem", &freqs);
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.fidelity.radiation, "bem");
    let label = result.fidelity.ignored.join(" ");
    assert!(
        label.contains("Boundary-element outside"),
        "label was {label}"
    );
    assert!(label.contains("outer shape"), "label was {label}");
    assert!(
        !label.contains("bounding box; curved"),
        "old AABB-only note still present: {label}"
    );
}

#[test]
fn a_front_driver_agrees_with_the_baffle_model_at_low_frequency() {
    // Below the mesh hand-over (~1 kHz for subdiv 4 on sealed-test), BEM and baffle
    // should stay within a couple of dB for a small front-facing sealed box.
    let freqs = [30.0, 60.0, 100.0, 200.0, 400.0];
    let baffle = run(&load_example("sealed-test"), "baffle", &freqs);
    let bem = run(&load_example("sealed-test"), "bem", &freqs);
    for (bin, freq) in freqs.iter().enumerate() {
        let apart = bem.summed_spl_db[bin] - baffle.summed_spl_db[bin];
        assert!(
            apart.abs() < 2.0,
            "{freq} Hz: bem vs baffle {apart:+.2} dB (bem={:.2} baffle={:.2})",
            bem.summed_spl_db[bin],
            baffle.summed_spl_db[bin]
        );
        assert!(
            apart != 0.0,
            "{freq} Hz was left as the baffle value; BEM should overwrite below the hand-over"
        );
    }
}

#[test]
fn a_tube_design_runs_under_bem_without_falling_back_to_errors() {
    let freqs = [50.0, 100.0, 200.0];
    let result = run(&load_example("pvc-tube"), "bem", &freqs);
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.fidelity.radiation, "bem");
    let label = result.fidelity.ignored.join(" ");
    assert!(label.contains("outer shape"), "label was {label}");
    for (bin, freq) in freqs.iter().enumerate() {
        assert!(
            result.summed_spl_db[bin].is_finite(),
            "{freq} Hz SPL was not finite"
        );
    }
}

/// CHK-BEM-8: a circular patch on a large flat face at low `ka` has self-impedance near the
/// flush analytic radiation load (README 5.4 / 5.6).
#[test]
fn chk_bem_8_mouth_self_impedance_matches_flush_piston_at_low_ka() {
    let registry = builtin();
    let fields = json!({
        "width_m": 4.0,
        "height_m": 4.0,
        "depth_m": 4.0,
        "wall_m": 0.02,
        "edge_radius_m": 0.0
    });
    let shape = registry
        .create_outer("box", fields.as_object().unwrap())
        .expect("box");
    let scene = RadiationScene {
        shape: shape.as_ref(),
        space: "full",
        baffle_width_m: 4.0,
        reference_m: [2.0, 0.0, 2.0],
    };
    let radius = 0.05;
    let area = PI * radius * radius;
    let mouth = SourcePatch {
        position_m: [2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        area_m2: area,
    };
    let ka = 0.25;
    let freq = ka * C0 / (2.0 * PI * radius);
    let table = bem::mouth_self_impedances(&scene, &[mouth], &[freq], 6).expect("Z_rad");
    let bem_z = table[0][0];
    let k = 2.0 * PI * freq / C0;
    let analytic = radiation_impedance(area, true, k);
    let re_err = (bem_z.re - analytic.re).abs() / analytic.re.max(1e-9);
    let im_err = (bem_z.im - analytic.im).abs() / analytic.im.abs().max(1e-9);
    assert!(
        bem_z.re > 0.0 && bem_z.im > 0.0,
        "Z_rad should be mass-like with positive resistance: {bem_z}"
    );
    assert!(
        re_err < 0.35 && im_err < 0.25,
        "ka={ka}: bem Z={bem_z:.4} analytic={analytic:.4} re_err={re_err:.3} im_err={im_err:.3}"
    );
}

#[test]
fn fixed_radiation_loads_match_the_analytic_leaf() {
    let area = PI * 0.02 * 0.02;
    let freq = 80.0;
    let k = 2.0 * PI * freq / C0;
    let z = radiation_impedance(area, false, k);
    let analytic = AcousticNode::Parallel(vec![
        AcousticNode::Compliance { volume_m3: 0.02 },
        AcousticNode::Duct {
            length_m: 0.12,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: 0.849 * 0.02,
            load: Box::new(AcousticNode::Radiation {
                area_m2: area,
                flush: false,
            }),
        },
    ]);
    let fixed = with_fixed_radiation_loads(&analytic, &[z]);
    let a = seatlock_acoustics::solve(&analytic, freq);
    let b = seatlock_acoustics::solve(&fixed, freq);
    assert!((a.z_in - b.z_in).norm() / a.z_in.norm().max(1e-12) < 1e-9);
    assert_eq!(a.openings.len(), 1);
    assert_eq!(b.openings.len(), 1);
}

#[test]
fn bem_joins_mouth_loads_into_the_waveguide_network() {
    let freqs = [40.0, 80.0, 160.0];
    let design = load_example("two-way-vented");
    let baffle = run_waveguide(&design, "baffle", &freqs);
    let bem = run_waveguide(&design, "bem", &freqs);
    assert!(
        bem.issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        bem.issues
    );
    let label = bem.fidelity.ignored.join(" ");
    assert!(
        label.contains("self-impedance") || label.contains(NETWORK_NOTE),
        "joined-network note missing: {label}"
    );
    // Joining BEM Z_rad must change the enclosure load the woofer sees vs analytic mouths.
    let z_b = &baffle.radiators[0].impedance_ohm;
    let z_m = &bem.radiators[0].impedance_ohm;
    let mut moved = false;
    for (bin, freq) in freqs.iter().enumerate() {
        let apart = (z_m[bin] - z_b[bin]).abs() / z_b[bin].max(1e-12);
        if apart > 1e-4 {
            moved = true;
        }
        assert!(
            z_m[bin].is_finite(),
            "{freq} Hz woofer impedance was not finite under bem"
        );
    }
    assert!(
        moved,
        "bem waveguide woofer Z matched analytic mouths at every bin; join did not take effect"
    );
}
