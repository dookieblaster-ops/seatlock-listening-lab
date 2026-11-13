//! Each driver sees the load of the chamber it pushes into, and drivers that share a chamber
//! share its air (README 5.2).

use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::{json, Value};

const W: f64 = 0.008;

/// A 0.16 x 0.40 x 0.18 m box with the given chambers, mounts at front heights `zs`, a
/// test-woofer-a on each, and the given ducts.
fn design(chambers: Value, zs: &[f64], rear: &[&str], ducts: Value) -> seatlock_core::DesignFile {
    let mounts: Vec<Value> = zs
        .iter()
        .zip(rear)
        .enumerate()
        .map(|(i, (z, r))| {
            json!({ "id": format!("m{i}"), "surface": "front", "at_m": [0.08, z],
                    "rear_into": r, "recess_m": 0.0 })
        })
        .collect();
    let radiators: Vec<Value> = (0..zs.len())
        .map(|i| json!({ "kind": "cone", "driver_id": "test-woofer-a", "mount": format!("m{i}") }))
        .collect();
    let chains: Vec<Value> = (0..zs.len())
        .map(|_| json!({ "blocks": [], "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null }))
        .collect();
    serde_json::from_value(json!({
        "schema_version": 1, "name": "chamber-loads",
        "speaker": {
            "enclosure": {
                "outer": { "kind": "box", "width_m": 0.16, "height_m": 0.40, "depth_m": 0.18,
                           "wall_m": W, "edge_radius_m": 0.0 },
                "chambers": chambers, "ducts": ducts, "mounts": mounts, "solids": [] },
            "radiators": radiators,
            "listening": { "distance_m": 1.0 } },
        "dsp": { "sample_rate_hz": 48000, "whole_speaker": [], "per_radiator": chains,
                 "correction": { "enabled": false, "protected_band_hz": [3000, 5000], "max_filters": 10 } },
        "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },
        "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": true, "extras_cost": 0.0 },
        "target": "hifi-default"
    }))
    .unwrap()
}

fn chamber(id: &str, z0: f64, z1: f64) -> Value {
    json!({ "id": id, "min_m": [W, W, z0], "max_m": [0.16 - W, 0.18 - W, z1], "fill": null })
}

fn worst(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

#[test]
fn two_drivers_in_one_chamber_each_see_half_of_it() {
    // Two drivers in a chamber of box volume B behave, each, as one driver in B / 2: each
    // takes its own displacement from the air, so the net air per driver is the same.
    let twin = design(
        json!([chamber("main", W, 0.392)]),
        &[0.1, 0.3],
        &["main", "main"],
        json!([]),
    );
    let single = design(
        json!([chamber("main", W, 0.2)]),
        &[0.1],
        &["main"],
        json!([]),
    );
    for enclosure in ["lumped", "waveguide"] {
        let request = SimulateRequest {
            enclosure: enclosure.into(),
            ..Default::default()
        };
        let a = simulate(&twin, &request, &builtin());
        let b = simulate(&single, &request, &builtin());
        assert!(
            a.issues
                .iter()
                .all(|i| i.severity != seatlock_core::issue::Severity::Error),
            "{:?}",
            a.issues
        );
        let d = worst(&a.radiators[0].spl_db, &b.radiators[0].spl_db);
        assert!(
            d < 0.01,
            "{enclosure}: each twin against the half box: {d} dB"
        );
        let d = worst(&a.radiators[1].spl_db, &b.radiators[0].spl_db);
        assert!(d < 0.01, "{enclosure}: second twin: {d} dB");
        assert!(a
            .fidelity
            .ignored
            .iter()
            .any(|line| line.starts_with("Drivers that share a chamber are the same model")));
    }
}

#[test]
fn a_driver_in_its_own_sealed_chamber_ignores_the_port_of_another() {
    // A woofer in a ported chamber below, and a second driver in its own sealed chamber above:
    // the second driver must behave exactly as it would alone in that chamber.
    let port = json!([{ "id": "port", "from": "main", "to": "outside",
        "path_m": [[0.08, 0.06, 0.04], [0.08, 0.0, 0.04]],
        "section": { "shape": "round", "start_m": [0.035], "end_m": [0.035] } }]);
    let both = design(
        json!([chamber("main", W, 0.25), chamber("mid", 0.258, 0.392)]),
        &[0.13, 0.325],
        &["main", "mid"],
        port,
    );
    let alone = design(
        json!([chamber("mid", 0.258, 0.392)]),
        &[0.325],
        &["mid"],
        json!([]),
    );
    for enclosure in ["lumped", "waveguide"] {
        let request = SimulateRequest {
            enclosure: enclosure.into(),
            ..Default::default()
        };
        let a = simulate(&both, &request, &builtin());
        let b = simulate(&alone, &request, &builtin());
        assert!(
            a.issues
                .iter()
                .all(|i| i.severity != seatlock_core::issue::Severity::Error),
            "{:?}",
            a.issues
        );
        let d = worst(&a.radiators[1].spl_db, &b.radiators[0].spl_db);
        assert!(
            d < 0.01,
            "{enclosure}: the upper driver differs by {d} dB from alone"
        );
        // And it has no port flow of its own.
        assert!(a.radiators[1].port_speed_m.iter().all(|v| *v == 0.0));
        assert!(a.radiators[0].port_speed_m.iter().any(|v| *v > 0.0));
    }
}
