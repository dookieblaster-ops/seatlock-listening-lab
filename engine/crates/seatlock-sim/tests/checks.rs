use seatlock_core::design_check;
use seatlock_sim::{builtin, simulate, SimulateRequest};

fn read_example() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    ))
    .unwrap()
}

#[test]
fn example_design_has_no_unknown_kinds() {
    let text = read_example();
    let checked = design_check(&text, &builtin()).unwrap();
    assert!(
        checked
            .issues
            .iter()
            .all(|issue| issue.code != "unsupported_kind"),
        "{:?}",
        checked.issues
    );
}

#[test]
fn chk_ext_2_unknown_radiator_is_left_out() {
    let text = r#"{
  "schema_version": 1,
  "name": "unknown-kind",
  "speaker": {
    "enclosure": {
      "outer": { "kind": "box", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18, "wall_m": 0.008, "edge_radius_m": 0.0 },
      "chambers": [],
      "ducts": [],
      "mounts": [{ "id": "m1", "surface": "front", "at_m": [0.08, 0.15], "rear_into": "main", "recess_m": 0.0 }],
      "solids": []
    },
    "radiators": [
      { "kind": "cone", "driver_id": "test-woofer-a", "mount": "m1" },
      { "kind": "experimental_panel", "driver_id": "x", "mount": "m1", "note": "keep me" }
    ],
    "listening": { "distance_m": 1.0 }
  },
  "dsp": {
    "sample_rate_hz": 48000,
    "whole_speaker": [],
    "per_radiator": [
      { "blocks": [{ "kind": "biquad", "type": "highpass", "f_hz": 35.0, "q": 0.707 }], "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null },
      { "blocks": [], "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null }
    ],
    "correction": { "enabled": true, "protected_band_hz": [3000.0, 5000.0], "max_filters": 10 }
  },
  "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },
  "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": true, "extras_cost": 0.0 },
  "target": "hifi-default"
}
"#;
    let registry = builtin();
    let checked = design_check(text, &registry).unwrap();
    let unknown: Vec<_> = checked
        .issues
        .iter()
        .filter(|issue| issue.code == "unsupported_kind")
        .collect();
    assert_eq!(unknown.len(), 1, "{:?}", checked.issues);
    assert!(unknown[0].message.contains("experimental_panel"));
    assert!(unknown[0].field == "speaker.radiators[1]");
    assert!(checked.canonical_json.contains("experimental_panel"));
    assert!(checked.canonical_json.contains("keep me"));
    let again = design_check(&checked.canonical_json, &registry).unwrap();
    assert_eq!(again.canonical_json, checked.canonical_json);

    let design = serde_json::from_str(text).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &registry);
    assert_eq!(result.radiators.len(), 1);
    assert_eq!(result.radiators[0].kind, "cone");
    assert!(result.radiators[0]
        .spl_db
        .iter()
        .any(|value| value.is_finite()));
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == "unsupported_kind"
            && issue.message.contains("experimental_panel")));
}

#[test]
fn sealed_test_simulates_and_reports_unknown_price() {
    let text = read_example();
    let design = serde_json::from_str(&text).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert_eq!(result.cost_note, "price unknown");
    assert!(result.interior_volume_m3.unwrap() > 0.0);
    assert!(result.summed_spl_db.iter().any(|value| *value > 0.0));
    eprintln!("SIM_MS {}", result.elapsed_ms);
}
