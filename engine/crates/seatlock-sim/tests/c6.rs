//! C6: Level-aware bass look-ahead from cone travel X/V.

use seatlock_sim::{builtin, correction_plan};
use serde_json::Value;

#[test]
fn c6_adds_dynamic_bass_on_a_cone_with_xmax() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let plan = correction_plan(&design, &builtin());
    let filters = plan["filters"].as_array().cloned().unwrap_or_default();
    let c6: Vec<_> = filters
        .iter()
        .filter(|f| f.get("kind").and_then(Value::as_str) == Some("dynamic_bass"))
        .collect();
    assert_eq!(c6.len(), 1, "expected one C6 block: {plan}");
    let block = c6[0];
    assert!(
        block.get("xmax_m").and_then(Value::as_f64).unwrap_or(0.0) > 0.0,
        "{block}"
    );
    assert!(
        block
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|r| r.contains("C6")),
        "{block}"
    );
    let message = plan["message"].as_str().unwrap_or("");
    assert!(
        message.contains("Level-aware bass (C6)"),
        "message missing C6: {message}"
    );
}

#[test]
fn c6_block_registers_and_reports_look_ahead_latency() {
    let mut fields = serde_json::Map::new();
    fields.insert("xmax_m".into(), serde_json::json!(0.004));
    fields.insert("re_ohm".into(), serde_json::json!(6.0));
    fields.insert("le_h".into(), serde_json::json!(0.0005));
    fields.insert("mms_kg".into(), serde_json::json!(0.012));
    fields.insert("cms_m_per_n".into(), serde_json::json!(0.0008));
    fields.insert("rms_ns_per_m".into(), serde_json::json!(1.0));
    fields.insert("bl_tm".into(), serde_json::json!(7.0));
    fields.insert("look_ahead_ms".into(), serde_json::json!(5.0));
    fields.insert("sample_rate_hz".into(), serde_json::json!(48_000.0));
    let registry = builtin();
    let block = registry
        .create_dsp("dynamic_bass", &fields)
        .expect("create dynamic_bass");
    assert_eq!(block.latency_samples(), 240);
}
