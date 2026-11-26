//! C10: Indirect-radiator tuning for a rear panel in a simple room.

use seatlock_sim::{builtin, correction_plan_with};
use serde_json::Value;

fn rear_panel() -> seatlock_core::DesignFile {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/rear-panel.seatlock.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn c10_sets_band_gain_and_zero_delay_on_the_rear_panel() {
    let design = rear_panel();
    let plan = correction_plan_with(&design, &builtin(), "baffle", "half");
    let plans = plan["per_radiator"].as_array().expect("per_radiator plans");
    assert!(plans.len() >= 2, "{plan}");
    let rear = &plans[1];
    let message = rear["message"].as_str().unwrap_or("");
    assert!(message.contains("Indirect radiator (C10)"), "{message}");
    assert_eq!(rear["delay_s"].as_f64(), Some(0.0));
    assert!(rear.get("gain_db").and_then(Value::as_f64).is_some());
    let filters = rear["filters"].as_array().unwrap();
    let hp = filters.iter().find(|f| {
        f.get("type").and_then(Value::as_str) == Some("highpass")
            && f.get("correction").and_then(Value::as_bool) == Some(true)
            && f.get("reason")
                .and_then(Value::as_str)
                .is_some_and(|r| r.contains("C10"))
    });
    assert!(hp.is_some(), "{filters:?}");
    let lp = filters.iter().find(|f| {
        f.get("type").and_then(Value::as_str) == Some("lowpass")
            && f.get("reason")
                .and_then(Value::as_str)
                .is_some_and(|r| r.contains("C10"))
    });
    assert!(lp.is_some(), "{filters:?}");
}

#[test]
fn c10_leaves_a_front_only_cabinet_alone() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let plan = correction_plan_with(&design, &builtin(), "baffle", "half");
    let plans = plan["per_radiator"].as_array().unwrap();
    let front = &plans[0];
    let message = front["message"].as_str().unwrap_or("");
    assert!(!message.contains("Indirect radiator (C10)"), "{message}");
    assert!(front.get("gain_db").is_none());
}
