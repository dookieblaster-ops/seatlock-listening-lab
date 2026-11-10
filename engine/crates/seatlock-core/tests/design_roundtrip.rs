use seatlock_core::design_check;

#[test]
fn sealed_test_roundtrips_unchanged() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    ))
    .expect("sealed-test design");
    let result = design_check(&text).expect("check");
    assert!(result
        .issues
        .iter()
        .all(|i| i.code.starts_with("WARN") || i.code.is_empty()));
    assert_eq!(result.canonical_json, text.trim());
}

#[test]
fn unknown_radiator_kind_warns_and_roundtrips() {
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
    "radiators": [{ "kind": "experimental_panel", "driver_id": "x", "mount": "m1" }],
    "listening": { "distance_m": 1.0 }
  },
  "dsp": {
    "sample_rate_hz": 48000,
    "whole_speaker": [],
    "per_radiator": [{
      "blocks": [{ "kind": "biquad", "type": "highpass", "f_hz": 35.0, "q": 0.707 }],
      "delay_s": 0.0,
      "gain_db": 0.0,
      "invert": false,
      "limiter": null
    }],
    "correction": { "enabled": true, "protected_band_hz": [3000, 5000], "max_filters": 10 }
  },
  "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },
  "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": true, "extras_cost": 0.0 },
  "target": "hifi-default"
}"#;
    let result = design_check(text).expect("check");
    let warn = result
        .issues
        .iter()
        .find(|i| i.code == "WARN_UNSUPPORTED_KIND")
        .expect("warning");
    assert!(warn.message.contains("experimental_panel"));
    assert_eq!(result.canonical_json, text.trim());
}
