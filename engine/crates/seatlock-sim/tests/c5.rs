//! C5: Cabinet-resonance cuts from a coarse full-3D pass on a sealed box.

use seatlock_sim::{builtin, correction_plan_with};
use serde_json::Value;

#[test]
fn c5_can_cut_standing_waves_on_a_tall_sealed_box() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    let mut design: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    // Stretch the box so the first height mode sits clearly in the midband.
    design["speaker"]["enclosure"]["outer"]["height_m"] = serde_json::json!(0.5);
    let design: seatlock_core::DesignFile = serde_json::from_value(design).unwrap();
    let plan = correction_plan_with(&design, &builtin(), "baffle", "half");
    let message = plan["message"].as_str().unwrap_or("");
    eprintln!("C5 plan message: {message}");
    let filters = plan["filters"].as_array().cloned().unwrap_or_default();
    let c5: Vec<_> = filters
        .iter()
        .filter(|f| {
            f.get("reason")
                .and_then(Value::as_str)
                .is_some_and(|r| r.contains("standing wave") && r.contains("damping"))
        })
        .collect();
    assert!(
        message.contains("Cabinet resonances (C5)") && !c5.is_empty(),
        "expected C5 standing-wave cuts on a tall empty sealed box: {plan}"
    );
}
