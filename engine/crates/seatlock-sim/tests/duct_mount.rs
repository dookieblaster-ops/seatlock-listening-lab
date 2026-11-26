//! A driver whose mount `rear_into` names a duct (README §6.2).

use seatlock_core::issue::Severity;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::json;

fn load(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn duct_mount_line_runs_under_waveguide_with_coupling_note() {
    let design = load("duct-mount-line");
    let report = simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            frequencies_hz: Some((20..=200).step_by(10).map(|f| f as f64).collect()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(
        report.issues.iter().all(|i| i.severity != Severity::Error),
        "{:?}",
        report.issues
    );
    assert!(
        report
            .fidelity
            .ignored
            .iter()
            .any(|n| n.contains("sits on duct") || n.contains("coupling")),
        "{:?}",
        report.fidelity.ignored
    );
    let near = report
        .frequencies_hz
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - 50.0).abs().partial_cmp(&(b.1 - 50.0).abs()).unwrap())
        .unwrap()
        .0;
    assert!(
        report.radiators[0].port_speed_m[near] > 0.01,
        "expected mouth flow at 50 Hz, got {}",
        report.radiators[0].port_speed_m[near]
    );
}

#[test]
fn same_coupling_volume_matches_a_listed_chamber_on_the_same_line() {
    // Build two designs that share one duct path: one with a listed chamber of volume V, one
    // with rear_into naming the duct and coupling_volume_m3 = V. Path stays outside the
    // chamber box so the tube does not steal chamber air.
    let mut listed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../../designs/examples/folded-line.seatlock.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let v = 0.002;
    // Chamber fits inside the 0.16×0.70×0.25 box (air ~0.012–0.238 in y) and stays clear of
    // the duct path (y ≤ 0.06).
    listed["speaker"]["enclosure"]["chambers"] = json!([{
        "id": "main",
        "min_m": [0.02, 0.13, 0.52],
        "max_m": [0.14, 0.23, 0.52 + v / (0.12 * 0.10)],
        "fill": null
    }]);
    listed["speaker"]["enclosure"]["ducts"][0]["path_m"] =
        json!([[0.08, 0.06, 0.60], [0.08, 0.06, 0.06], [0.08, 0.0, 0.06]]);
    listed["speaker"]["enclosure"]["mounts"][0]["rear_into"] = json!("main");
    listed["speaker"]["enclosure"]["mounts"][0]["at_m"] = json!([0.08, 0.60]);

    let mut mounted = listed.clone();
    mounted["name"] = json!("duct-mount-same-v");
    mounted["speaker"]["enclosure"]["chambers"] = json!([]);
    mounted["speaker"]["enclosure"]["mounts"][0]["rear_into"] = json!("line");
    mounted["speaker"]["enclosure"]["mounts"][0]["coupling_volume_m3"] = json!(v);

    let listed: seatlock_core::DesignFile = serde_json::from_value(listed).unwrap();
    let mounted: seatlock_core::DesignFile = serde_json::from_value(mounted).unwrap();
    let freqs: Vec<f64> = (20..=120).step_by(10).map(|f| f as f64).collect();
    let req = SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        frequencies_hz: Some(freqs.clone()),
        ..SimulateRequest::default()
    };
    let a = simulate(&listed, &req, &builtin());
    let b = simulate(&mounted, &req, &builtin());
    assert!(
        a.issues
            .iter()
            .chain(b.issues.iter())
            .all(|i| i.severity != Severity::Error),
        "listed {:?} mounted {:?}",
        a.issues,
        b.issues
    );
    for (i, f) in freqs.iter().enumerate() {
        let da = a.radiators[0].spl_db[i];
        let db = b.radiators[0].spl_db[i];
        assert!(
            (da - db).abs() < 0.35,
            "at {f} Hz listed {da:.2} vs duct-mount {db:.2}"
        );
    }
}

#[test]
fn offset_tap_notes_and_differs_from_closed_end() {
    // King / Augspurger: driver part-way down the line (tap_fraction from the closed end).
    let closed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../../designs/examples/duct-mount-line.seatlock.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let mut offset = closed.clone();
    offset["name"] = json!("duct-mount-offset");
    offset["speaker"]["enclosure"]["mounts"][0]["tap_fraction"] = json!(0.2);

    let closed: seatlock_core::DesignFile = serde_json::from_value(closed).unwrap();
    let offset: seatlock_core::DesignFile = serde_json::from_value(offset).unwrap();
    let freqs: Vec<f64> = (30..=300).step_by(5).map(|f| f as f64).collect();
    let req = SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        frequencies_hz: Some(freqs.clone()),
        ..SimulateRequest::default()
    };
    let a = simulate(&closed, &req, &builtin());
    let b = simulate(&offset, &req, &builtin());
    assert!(
        a.issues
            .iter()
            .chain(b.issues.iter())
            .all(|i| i.severity != Severity::Error),
        "closed {:?} offset {:?}",
        a.issues,
        b.issues
    );
    assert!(
        b.fidelity
            .ignored
            .iter()
            .any(|n| n.contains("offset") || n.contains("closed stub")),
        "{:?}",
        b.fidelity.ignored
    );
    // Offset must change the mouth output: max |ΔSPL| over the band is not tiny.
    let mut max_delta = 0.0_f64;
    for i in 0..freqs.len() {
        max_delta = max_delta.max((a.radiators[0].spl_db[i] - b.radiators[0].spl_db[i]).abs());
    }
    assert!(
        max_delta > 1.0,
        "expected offset tap to move SPL by >1 dB somewhere, max Δ={max_delta:.2}"
    );
}

#[test]
fn tap_zero_matches_omitted_tap_fraction() {
    let mut with: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../../designs/examples/duct-mount-line.seatlock.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let without = with.clone();
    with["speaker"]["enclosure"]["mounts"][0]["tap_fraction"] = json!(0.0);
    let with: seatlock_core::DesignFile = serde_json::from_value(with).unwrap();
    let without: seatlock_core::DesignFile = serde_json::from_value(without).unwrap();
    let freqs: Vec<f64> = (20..=120).step_by(10).map(|f| f as f64).collect();
    let req = SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        frequencies_hz: Some(freqs.clone()),
        ..SimulateRequest::default()
    };
    let a = simulate(&with, &req, &builtin());
    let b = simulate(&without, &req, &builtin());
    for (i, f) in freqs.iter().enumerate() {
        let da = a.radiators[0].spl_db[i];
        let db = b.radiators[0].spl_db[i];
        assert!(
            (da - db).abs() < 0.05,
            "at {f} Hz tap=0 {da:.3} vs omitted {db:.3}"
        );
    }
}
