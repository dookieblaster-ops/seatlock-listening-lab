//! The `bem` radiation model inside `simulate` (README section 5.6).

use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::Value;

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
