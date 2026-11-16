//! The `bem` radiation model inside `simulate` (README section 5.6).

use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::Value;

fn sealed_test() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
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
fn bem_is_registered_and_labels_the_box_mesh() {
    let freqs = [50.0, 100.0, 200.0];
    let result = run(&sealed_test(), "bem", &freqs);
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
    assert!(label.contains("bounding box"), "label was {label}");
}

#[test]
fn a_front_driver_agrees_with_the_baffle_model_at_low_frequency() {
    // Below the box-mesh hand-over (~1 kHz for subdiv 4 on sealed-test), BEM and baffle
    // should stay within a couple of dB for a small front-facing sealed box.
    let freqs = [30.0, 60.0, 100.0, 200.0, 400.0];
    let baffle = run(&sealed_test(), "baffle", &freqs);
    let bem = run(&sealed_test(), "bem", &freqs);
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
