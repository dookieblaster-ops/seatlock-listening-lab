use seatlock_core::constants::{C0, RHO0};
use seatlock_ext_example::register;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use std::f64::consts::PI;

#[test]
fn chk_ext_1_registered_kinds_change_the_result() {
    let mut registry = builtin();
    register(&mut registry);

    let text = r#"{
      "schema_version": 1,
      "name": "extension",
      "speaker": {
        "enclosure": {
          "outer": { "kind": "test_cylinder", "radius_m": 0.1, "height_m": 0.2 },
          "chambers": [],
          "ducts": [],
          "mounts": [],
          "solids": []
        },
        "radiators": [{ "kind": "test_constant_source", "u_per_volt": 0.001 }],
        "listening": { "distance_m": 1.0 }
      },
      "dsp": {
        "sample_rate_hz": 48000,
        "whole_speaker": [],
        "per_radiator": [{
          "blocks": [{ "kind": "test_half_gain" }],
          "delay_s": 0.0,
          "gain_db": 0.0,
          "invert": false,
          "limiter": null
        }],
        "correction": { "enabled": false, "protected_band_hz": [3000.0, 5000.0], "max_filters": 0 }
      },
      "electronics": { "amplifiers": [], "dsp_device": null },
      "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": false, "extras_cost": 0.0 },
      "target": "hifi-default"
    }"#;
    let design = serde_json::from_str(text).unwrap();
    let request = SimulateRequest {
        voltage_v: 1.0,
        frequencies_hz: Some(vec![100.0]),
        space: "half".to_string(),
        ..SimulateRequest::default()
    };
    let result = simulate(&design, &request, &registry);
    let expected_volume = PI * 0.1 * 0.1 * 0.2;
    assert!((result.interior_volume_m3.unwrap() - expected_volume).abs() < 1e-12);
    let omega = 2.0 * PI * 100.0;
    let velocity = 0.001 * 0.5;
    let pressure = omega * RHO0 * velocity * 2.0 / (4.0 * PI * 1.0);
    let spl = 20.0 * (pressure / 20e-6).log10();
    assert!(
        (result.summed_spl_db[0] - spl).abs() < 0.05,
        "{} vs {spl}",
        result.summed_spl_db[0]
    );
    let _ = C0;
}
