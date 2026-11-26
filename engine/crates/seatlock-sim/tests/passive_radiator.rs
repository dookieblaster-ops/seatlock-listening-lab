//! Passive radiator as a Small Map/Cap/Rap outside branch (README §5).

use seatlock_acoustics::{network::lumped_network, sealed_with_pr, PassiveRadiatorMech};
use seatlock_core::catalog;
use seatlock_core::issue::Severity;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use std::f64::consts::PI;

fn dsa175_pr() -> PassiveRadiatorMech {
    let record = catalog::passive_radiator("dayton-dsa175-pr").expect("catalog PR");
    let p = record.get("published").unwrap();
    PassiveRadiatorMech {
        mms_kg: p["mms_kg"].as_f64().unwrap(),
        cms_m_n: p["cms_m_per_n"].as_f64().unwrap(),
        rms_ns_m: p["rms_ns_per_m"].as_f64().unwrap(),
        sd_m2: p["sd_m2"].as_f64().unwrap(),
    }
}

#[test]
fn catalog_pr_resonance_matches_sheet_fs() {
    let pr = dsa175_pr();
    let fs = 1.0 / (2.0 * PI * (pr.mms_kg * pr.cms_m_n).sqrt());
    assert!((fs - 26.8).abs() < 0.05, "fs {fs}");
}

#[test]
fn lumped_network_matches_sealed_with_pr() {
    let pr = dsa175_pr();
    let volume = 0.022;
    let duct = seatlock_acoustics::network::Duct {
        from: 0,
        to: None,
        area_m2: pr.sd_m2,
        length_eff_m: 0.0,
        passive: Some(pr),
    };
    for freq in [20.0, 40.0, 63.0, 100.0, 200.0] {
        let (z_ref, h_ref) = sealed_with_pr(volume, pr, freq);
        let solved = lumped_network(&[volume], &[duct], 0, freq);
        let z_err = (solved.z_ab - z_ref).norm() / z_ref.norm();
        let h_err = (solved.h_out[0] - h_ref).norm();
        assert!(z_err < 1e-12, "at {freq} Hz Z rel {z_err}");
        assert!(h_err < 1e-12, "at {freq} Hz H err {h_err}");
    }
}

#[test]
fn example_pr_box_runs_and_prices_the_radiator() {
    let path = format!(
        "{}/../../../designs/examples/pr-box.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let report = simulate(
        &design,
        &SimulateRequest {
            enclosure: "lumped".into(),
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
            .any(|n| n.contains("Passive radiator") || n.contains("Map")),
        "{:?}",
        report.fidelity.ignored
    );
    assert!(
        report
            .cost
            .lines
            .iter()
            .any(|line| line.item.contains("DSA175-PR") || line.item.contains("dsa175-pr")),
        "{:?}",
        report.cost.lines
    );
    // Near the PR free-air resonance the PR branch should move air (port speed > 0).
    let near = report
        .frequencies_hz
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - 26.8).abs().partial_cmp(&(b.1 - 26.8).abs()).unwrap())
        .unwrap()
        .0;
    let speed = report.radiators[0].port_speed_m[near];
    assert!(
        speed > 0.01,
        "expected PR motion near fs, got port_speed {speed} at bin {near}"
    );
}

#[test]
fn waveguide_pr_box_matches_lumped_within_a_tenth_db() {
    let path = format!(
        "{}/../../../designs/examples/pr-box.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let freqs: Vec<f64> = (20..=200).step_by(10).map(|f| f as f64).collect();
    let lumped = simulate(
        &design,
        &SimulateRequest {
            enclosure: "lumped".into(),
            radiation: "baffle".into(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let wave = simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(
        lumped
            .issues
            .iter()
            .chain(wave.issues.iter())
            .all(|i| i.severity != Severity::Error),
        "lumped {:?} waveguide {:?}",
        lumped.issues,
        wave.issues
    );
    // Waveguide should not fall back with the old "detailed duct model has no PR" note.
    assert!(
        !wave
            .fidelity
            .ignored
            .iter()
            .any(|n| n.contains("detailed duct model has no PR")
                || n.contains("keeps the quick network")),
        "{:?}",
        wave.fidelity.ignored
    );
    for (i, f) in freqs.iter().enumerate() {
        let da = lumped.radiators[0].spl_db[i];
        let db = wave.radiators[0].spl_db[i];
        assert!(
            (da - db).abs() < 0.1,
            "at {f} Hz lumped {da:.2} vs waveguide {db:.2}"
        );
    }
}
