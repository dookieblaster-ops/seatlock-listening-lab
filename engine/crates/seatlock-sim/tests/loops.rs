//! Coupled chambers that form a loop in the detailed model (README §5.4).

use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::json;

#[test]
fn a_looped_two_chamber_cabinet_uses_the_detailed_mesh_not_the_quick_fallback() {
    let path = format!(
        "{}/../../../designs/examples/two-chamber.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut design: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    // Second coupler between front and rear: the ducts now form a loop.
    let mut ducts = design["speaker"]["enclosure"]["ducts"]
        .as_array()
        .unwrap()
        .clone();
    let mut extra = ducts[0].clone();
    extra["id"] = json!("coupler-b");
    extra["path_m"] = json!([[0.137, 0.13, 0.36], [0.137, 0.18, 0.36]]);
    ducts.push(extra);
    design["speaker"]["enclosure"]["ducts"] = json!(ducts);
    let design: seatlock_core::DesignFile = serde_json::from_value(design).unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
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
            .any(|l| l.contains("form a loop") && l.contains("mesh")),
        "{:?}",
        result.fidelity.ignored
    );
    assert!(
        result
            .fidelity
            .ignored
            .iter()
            .all(|l| !l.contains("cannot solve this cabinet")),
        "should not fall back to the quick model: {:?}",
        result.fidelity.ignored
    );
}
