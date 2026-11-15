//! Any number of drivers and vents (no fixed cap of four).

use seatlock_sim::{builtin, listen_package, simulate, SimulateRequest};
use serde_json::json;

fn duct(index: usize) -> serde_json::Value {
    let x = 0.03 + index as f64 * 0.02;
    json!({
        "id": format!("vent-{index}"),
        "from": "main",
        "to": "outside",
        "path_m": [[x, 0.08, 0.04], [x, 0.0, 0.04]],
        "section": { "shape": "round", "start_m": [0.02], "end_m": [0.02] }
    })
}

#[test]
fn six_vents_and_three_drivers_simulate() {
    let ducts: Vec<_> = (0..6).map(duct).collect();
    let mounts: Vec<_> = (0..3)
        .map(|i| {
            json!({
                "id": format!("m{i}"),
                "surface": "front",
                "at_m": [0.05 + i as f64 * 0.04, 0.12],
                "rear_into": "main",
                "recess_m": 0.0
            })
        })
        .collect();
    let radiators: Vec<_> = (0..3)
        .map(|i| json!({ "kind": "cone", "driver_id": "test-woofer-a", "mount": format!("m{i}") }))
        .collect();
    let chains: Vec<_> = (0..3)
        .map(|_| json!({ "blocks": [], "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null }))
        .collect();
    let design: seatlock_core::DesignFile = serde_json::from_value(json!({
        "schema_version": 1,
        "name": "many-openings",
        "speaker": {
            "enclosure": {
                "outer": { "kind": "box", "width_m": 0.28, "height_m": 0.36, "depth_m": 0.22,
                           "wall_m": 0.012, "edge_radius_m": 0.0 },
                "chambers": [],
                "ducts": ducts,
                "mounts": mounts,
                "solids": []
            },
            "radiators": radiators,
            "listening": { "distance_m": 1.0 }
        },
        "dsp": {
            "sample_rate_hz": 48000,
            "whole_speaker": [],
            "per_radiator": chains,
            "correction": { "enabled": false, "protected_band_hz": [3000.0, 5000.0], "max_filters": 8 }
        },
        "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },
        "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": false, "extras_cost": 0.0 },
        "target": "hifi-default"
    }))
    .unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(vec![80.0]),
            radiation: "point".into(),
            walls: false,
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(
        result
            .issues
            .iter()
            .all(|issue| issue.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.radiators.len(), 3);
    assert!(result.summed_spl_db[0].is_finite());
    let pack = listen_package(&design, &builtin(), 48_000.0, false, 0);
    assert_eq!(pack["channels"].as_array().unwrap().len(), 3);
}
