use seatlock_core::{design_check, Registry};

fn example_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    )
    .to_string()
}

#[test]
fn sealed_test_roundtrips_unchanged() {
    let text = std::fs::read_to_string(example_path()).unwrap();
    let result = design_check(&text, &Registry::new()).unwrap();
    assert_eq!(result.canonical_json, text);
}

#[test]
fn unknown_fields_roundtrip() {
    let text = r#"{
  "schema_version": 1,
  "name": "extra",
  "future_note": "keep",
  "speaker": {
    "enclosure": {
      "outer": { "kind": "box", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18, "wall_m": 0.008, "edge_radius_m": 0.0 },
      "chambers": [],
      "ducts": [],
      "mounts": [{ "id": "m1", "surface": "front", "at_m": [0.08, 0.15], "rear_into": "main", "recess_m": 0.0 }],
      "solids": []
    },
    "radiators": [{ "kind": "cone", "driver_id": "test-woofer-a", "mount": "m1", "bonus": true }],
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
    "correction": { "enabled": true, "protected_band_hz": [3000.0, 5000.0], "max_filters": 10 }
  },
  "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },
  "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": true, "extras_cost": 0.0 },
  "target": "hifi-default"
}
"#;
    let registry = Registry::new();
    let once = design_check(text, &registry).unwrap();
    assert!(once.canonical_json.contains("future_note"));
    assert!(once.canonical_json.contains("bonus"));
    let twice = design_check(&once.canonical_json, &registry).unwrap();
    assert_eq!(twice.canonical_json, once.canonical_json);
}
