//! FIR DSP blocks and latency notes (README §7 Level 3).

use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::json;

#[test]
fn linear_phase_crossover_notes_latency_on_the_fidelity_label() {
    let path = format!(
        "{}/../../../designs/examples/sealed-test.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut design: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    design["dsp"]["per_radiator"][0]["blocks"] = json!([{
        "kind": "crossover",
        "family": "linear-phase",
        "order": 4,
        "band": "low",
        "f_hz": 2000.0
    }]);
    let design: seatlock_core::DesignFile = serde_json::from_value(design).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert!(
        result
            .fidelity
            .ignored
            .iter()
            .any(|line| line.contains("DSP latency") && line.contains("64 samples")),
        "expected latency note, got {:?}",
        result.fidelity.ignored
    );
}
