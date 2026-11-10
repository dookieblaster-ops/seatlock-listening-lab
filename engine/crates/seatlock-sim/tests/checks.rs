use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_core::design_check;
use seatlock_core::grid::log_grid;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use std::f64::consts::PI;

fn load_fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

fn spl_at(design_json: &str, frequencies: &[f64]) -> (f64, Vec<seatlock_sim::RadiatorCurve>) {
    let design = serde_json::from_str(design_json).unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            space: "half".to_string(),
            voltage_v: 2.83,
            frequencies_hz: Some(frequencies.to_vec()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(
        (result.interior_volume_m3.unwrap() - 0.005).abs() < 1e-9,
        "volume {}",
        result.interior_volume_m3.unwrap()
    );
    (result.radiators[0].spl_db[0], result.radiators)
}

fn reference_half_space_db() -> f64 {
    let fs: f64 = 50.0;
    let qes: f64 = 0.45;
    let vas: f64 = 0.010;
    let re: f64 = 6.0;
    let voltage: f64 = 2.83;
    let eta0 = 4.0 * PI * PI * fs.powi(3) * vas / (C0.powi(3) * qes);
    let power = RHO0 * C0 * eta0 * (voltage * voltage / re) / (2.0 * PI);
    10.0 * (power / (P_REF * P_REF)).log10()
}

#[test]
fn chk_cost_1_sealed_test_hand_calculation() {
    // Hand calculation for designs/examples/sealed-test.seatlock.json
    // outer = 0.160 * 0.250 * 0.180
    // inner = (0.160 - 2*0.008) * (0.250 - 2*0.008) * (0.180 - 2*0.008)
    // hole = pi * (0.118/2)^2 * 0.008
    // solid = outer - inner - hole
    // mass = solid * 1240 kg/m^3 * infill 1
    // driver, filament, and amplifier prices are null, so the price is unknown
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let outer = 0.160 * 0.250 * 0.180;
    let air = (0.160 - 2.0 * 0.008) * (0.250 - 2.0 * 0.008) * (0.180 - 2.0 * 0.008);
    let hole = std::f64::consts::PI * (0.118_f64 / 2.0).powi(2) * 0.008;
    let solid = outer - air - hole;
    let mass = solid * 1240.0;
    assert!((report.interior_volume_m3 - air).abs() < 1e-12);
    assert!(
        (report.solid_volume_m3 - solid).abs() < 1e-12,
        "{}",
        report.solid_volume_m3
    );
    assert!((report.print_mass_kg - mass).abs() < 1e-9);
    assert_eq!(report.cost_note, "price unknown");
    assert!(
        (report.voxel_air_volume_m3 - report.interior_volume_m3).abs()
            <= 0.02 * report.interior_volume_m3
    );
}

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

#[test]
fn chk_drv_2_and_chk_box_1_through_simulate() {
    let design = load_fixture("chk-box-le0");
    let level = reference_half_space_db();
    assert!((level - 87.75).abs() < 0.02);
    let (spl, _) = spl_at(&design, &[1_000.0]);
    assert!((spl - level).abs() <= 0.02, "drv2 {spl} vs {level}");

    let fc = 50.0 * 3.0_f64.sqrt();
    let qts = 0.45 * 4.0 / (0.45 + 4.0);
    let qtc = qts * 3.0_f64.sqrt();
    assert!((fc - 86.60).abs() < 0.01);
    assert!((qtc - 0.7006).abs() < 0.0001);
    let freqs = log_grid(5.0, 20_000.0, 12.0);
    let (_, curves) = spl_at(&design, &freqs);
    for (freq, spl) in freqs.iter().zip(&curves[0].spl_db) {
        let s = num_complex::Complex64::new(0.0, freq / fc);
        let highpass = (s * s) / (s * s + s / qtc + 1.0);
        let expected = level + 20.0 * highpass.norm().log10();
        assert!(
            (spl - expected).abs() <= 0.001,
            "{freq} Hz {spl} vs {expected}"
        );
    }
}

#[test]
fn chk_box_2_through_simulate() {
    let design = load_fixture("chk-box-published");
    let freqs = [40.0, 86.60, 1_000.0];
    let (_, curves) = spl_at(&design, &freqs);
    let curve = &curves[0];
    let spl = [74.02_f64, 84.66, 87.12];
    let ohms = [7.87_f64, 59.33, 6.50];
    let travel_m = [1.381e-3_f64, 1.003e-3, 0.0100e-3];
    for i in 0..3 {
        assert!(
            (curve.spl_db[i] - spl[i]).abs() <= 0.02,
            "spl {}",
            curve.spl_db[i]
        );
        let z_tol = ohms[i].abs() * 0.005;
        assert!(
            (curve.impedance_ohm[i] - ohms[i]).abs() <= z_tol,
            "z {}",
            curve.impedance_ohm[i]
        );
        let x_tol = travel_m[i].abs() * 0.005;
        assert!(
            (curve.cone_travel_m[i] - travel_m[i]).abs() <= x_tol,
            "x {}",
            curve.cone_travel_m[i]
        );
    }
}

#[test]
fn unknown_request_space_uses_its_own_code() {
    let design =
        serde_json::from_str::<seatlock_core::DesignFile>(&load_fixture("chk-box-le0")).unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            space: "sphere".to_string(),
            frequencies_hz: Some(vec![100.0]),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == "unsupported_request"));
    assert!(result
        .issues
        .iter()
        .all(|issue| issue.field != "request.space" || issue.code == "unsupported_request"));
}
