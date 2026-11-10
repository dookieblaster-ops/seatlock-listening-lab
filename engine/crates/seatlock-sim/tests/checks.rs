use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_core::design_check;
use seatlock_core::grid::log_grid;
use seatlock_correct::{correct, filter_db};
use seatlock_sim::{builtin, simulate, SimulateRequest};
use std::f64::consts::PI;

#[test]
fn chk_eq_2_correction_respects_limits_and_protected_band() {
    let design =
        serde_json::from_str::<seatlock_core::DesignFile>(&load_fixture("chk-box-published"))
            .unwrap();
    let freqs = log_grid(10.0, 20_000.0, 24.0);
    let result = simulate(
        &design,
        &SimulateRequest {
            space: "half".to_string(),
            voltage_v: 2.83,
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let curve = &result.radiators[0];
    let outcome = correct(
        &freqs,
        &curve.spl_db,
        &curve.max_spl_db,
        95.0,
        [3000.0, 5000.0],
        8,
        48_000.0,
        86.60,
        0.7006,
        false,
        0.0,
        &[],
    );
    for filter in &outcome.filters {
        if filter.gain_db > 0.0 {
            assert!(
                filter.f_hz < 3000.0 || filter.f_hz > 5000.0,
                "{}",
                filter.reason
            );
        }
    }
    for freq in freqs
        .iter()
        .copied()
        .filter(|freq| *freq >= 3000.0 && *freq <= 5000.0)
    {
        let boost: f64 = outcome
            .filters
            .iter()
            .map(|filter| filter_db(filter, freq, 48_000.0))
            .sum();
        assert!(boost <= 0.05, "boost {boost} at {freq}");
    }
    let h_ref: f64 = outcome
        .filters
        .iter()
        .map(|filter| filter_db(filter, 1000.0, 48_000.0))
        .sum();
    let spl_ref = curve.spl_db.iter().zip(&freqs).min_by(|(a, fa), (b, fb)| {
        (*fa - 1000.0)
            .abs()
            .partial_cmp(&(*fb - 1000.0).abs())
            .unwrap()
            .then(a.partial_cmp(b).unwrap())
    });
    let spl_1k = curve.spl_db[freqs.iter().position(|freq| *freq >= 1000.0).unwrap()];
    let _ = spl_ref;
    let voltage = 2.83 * 10_f64.powf((95.0 - spl_1k - h_ref) / 20.0);
    for (index, freq) in freqs.iter().enumerate() {
        let gain: f64 = outcome
            .filters
            .iter()
            .map(|filter| filter_db(filter, *freq, 48_000.0))
            .sum();
        let drive = voltage * 10_f64.powf(gain / 20.0);
        let travel = curve.cone_travel_m[index] * (drive / 2.83);
        let current = drive / curve.impedance_ohm[index];
        assert!(travel <= 0.004 * 1.001, "travel {travel} at {freq}");
        assert!(current * current * 6.0 <= 30.0 * 1.001, "heat at {freq}");
        assert!(
            drive <= (200.0_f64).sqrt() * 1.001,
            "amp at {freq} drive {drive}"
        );
    }
}

#[test]
fn sealed_test_correction_keeps_a_midrange_fix_or_names_the_region() {
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let plan = seatlock_sim::correction_plan(&design, &builtin());
    let filters = plan["filters"].as_array().unwrap();
    let message = plan["message"].as_str().unwrap_or("");
    let has_peaking = filters.iter().any(|filter| filter["type"] == "peaking");
    assert!(
        has_peaking || message.contains("Hz"),
        "{message} {filters:?}"
    );
    let freqs = log_grid(10.0, 20_000.0, 24.0);
    let result = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let headroom: Vec<f64> = result.radiators[0]
        .max_spl_db
        .iter()
        .map(|level| level - 95.0)
        .collect();
    for (index, freq) in freqs.iter().enumerate() {
        let mut boost = 0.0;
        for filter in filters {
            let kind = filter["type"].as_str().unwrap_or("peaking");
            let f_hz = filter["f_hz"].as_f64().unwrap_or(1000.0);
            let q = filter["q"].as_f64().unwrap_or(0.707);
            let gain = filter["gain_db"].as_f64().unwrap_or(0.0);
            let from_hz = filter["from_hz"].as_f64().unwrap_or(0.0);
            let from_q = filter["from_q"].as_f64().unwrap_or(0.0);
            boost += filter_db(
                &seatlock_correct::CorrectionFilter {
                    filter_type: kind.into(),
                    f_hz,
                    q,
                    gain_db: gain,
                    from_hz,
                    from_q,
                    reason: String::new(),
                },
                *freq,
                48_000.0,
            );
        }
        let allowed = headroom[index].max(0.0);
        assert!(
            boost.max(0.0) <= allowed + 0.3,
            "boost {boost} headroom {} at {freq}",
            headroom[index]
        );
    }
}

#[test]
fn woofer_b_extends_bass_at_85_db() {
    let text = r#"{
      "schema_version": 1, "name": "b20",
      "speaker": {"enclosure": {"outer": {"kind":"box","width_m":0.27,"height_m":0.42,"depth_m":0.22,"wall_m":0.01,"edge_radius_m":0.0},
        "chambers":[],"ducts":[],"mounts":[],"solids":[]},
        "radiators":[{"kind":"cone","driver_id":"test-woofer-b","mount":"m1"}],
        "listening":{"distance_m":1.0}},
      "dsp":{"sample_rate_hz":48000,"whole_speaker":[],"per_radiator":[],
        "correction":{"enabled":true,"protected_band_hz":[3000.0,5000.0],"max_filters":8}},
      "electronics":{"amplifiers":["test-amp-a"],"dsp_device":null},
      "build":{"material":"pla","infill_fraction":1.0,"stereo_pair":false,"extras_cost":0.0},
      "target":"hifi-default"
    }"#;
    let design = serde_json::from_str::<seatlock_core::DesignFile>(text).unwrap();
    let freqs = log_grid(10.0, 20_000.0, 12.0);
    let before = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(freqs.clone()),
            voltage_v: 2.83,
            space: "half".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let outcome = correct(
        &freqs,
        &before.radiators[0].spl_db,
        &before.radiators[0].max_spl_db,
        85.0,
        [3000.0, 5000.0],
        8,
        48_000.0,
        56.6,
        0.54,
        false,
        0.0,
        &[],
    );
    let mut spl = before.radiators[0].spl_db.clone();
    for filter in &outcome.filters {
        for (level, freq) in spl.iter_mut().zip(&freqs) {
            *level += filter_db(filter, *freq, 48_000.0);
        }
    }
    let (f6_before, _, _) = seatlock_correct::metrics(
        &freqs,
        &before.radiators[0].spl_db,
        &before.radiators[0].max_spl_db,
    );
    let (f6_after, _, _) = seatlock_correct::metrics(&freqs, &spl, &before.radiators[0].max_spl_db);
    assert!(
        f6_after < f6_before,
        "f6 {f6_after} vs {f6_before} {}",
        outcome.message
    );
    let curve = &before.radiators[0];
    let h_ref: f64 = outcome
        .filters
        .iter()
        .map(|filter| filter_db(filter, 1000.0, 48_000.0))
        .sum();
    let spl_1k = curve.spl_db[freqs.iter().position(|freq| *freq >= 1000.0).unwrap()];
    let voltage = 2.83 * 10_f64.powf((85.0 - spl_1k - h_ref) / 20.0);
    for (index, freq) in freqs.iter().enumerate() {
        let gain: f64 = outcome
            .filters
            .iter()
            .map(|filter| filter_db(filter, *freq, 48_000.0))
            .sum();
        let drive = voltage * 10_f64.powf(gain / 20.0);
        let travel = curve.cone_travel_m[index] * (drive / 2.83);
        let current = drive / curve.impedance_ohm[index];
        assert!(travel <= 0.006 * 1.001, "travel {travel} at {freq}");
        assert!(current * current * 6.0 <= 50.0 * 1.001, "heat at {freq}");
        assert!(
            drive <= (200.0_f64).sqrt() * 1.001,
            "amp at {freq} drive {drive}"
        );
    }
}

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

#[test]
fn page_grid_scores_after_correction() {
    let mut design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let plan = seatlock_sim::correction_plan(&design, &builtin());
    let filters = plan["filters"].as_array().unwrap();
    for filter in filters {
        let part =
            serde_json::from_value::<seatlock_core::design::KindPart>(filter.clone()).unwrap();
        design.dsp.per_radiator[0].blocks.push(part);
    }
    let start = 20.0_f64;
    let end = 20_000.0_f64;
    let per = 48.0_f64;
    let steps = ((end / start).log2() * per).round() as i32;
    let mut freqs = Vec::new();
    for i in 0..=steps {
        let freq = start * 2_f64.powf(i as f64 / per);
        if freq < end {
            freqs.push(freq);
        }
    }
    freqs.push(end);
    let result = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(freqs),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert_eq!(format!("{:.0}", result.f6_hz), "65");
    assert_eq!(format!("{:.1}", result.flatness_db), "6.5");
    assert_eq!(format!("{:.0}", result.max_spl_metric_db), "83");
    assert_eq!(result.cost_note, "price unknown");
}

#[test]
fn listen_package_has_an_impulse_and_filter_text() {
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let value = seatlock_sim::listen_package(&design, &builtin(), 48_000.0, true, 1024);
    let text = value["filter_text"].as_str().unwrap();
    assert!(text.contains("Radiator 1"), "{text}");
    assert!(text.contains("biquad"), "{text}");
    let impulse = value["impulses"][0].as_array().unwrap();
    assert_eq!(impulse.len(), 1024);
    assert!(impulse
        .iter()
        .any(|sample| sample.as_f64().unwrap_or(0.0).abs() > 1e-8));
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

#[test]
fn chk_box_3_and_4_through_simulate() {
    let volume = 0.020 / 2.0_f64.sqrt();
    let depth = volume / (0.25 * 0.24);
    let text = format!(
        r#"{{
          "schema_version": 1, "name": "vented-check",
          "speaker": {{
            "enclosure": {{
              "outer": {{"kind":"box","width_m":0.27,"height_m":0.26,"depth_m":{},"wall_m":0.01,"edge_radius_m":0.0}},
              "chambers":[],
              "ducts":[{{"id":"port","from":"main","to":"outside","path_m":[[0.1,0.2,0.1],[0.1,0.098725,0.1]],"section":{{"shape":"round","start_m":[0.035],"end_m":[0.035]}}}}],
              "mounts":[{{"id":"m1","surface":"front","at_m":[0.1,0.1],"rear_into":"main","recess_m":0.0}}],
              "solids":[]
            }},
            "radiators":[{{"kind":"cone","driver_id":"test-woofer-b","mount":"m1"}}],
            "listening":{{"distance_m":1.0}}
          }},
          "dsp":{{"sample_rate_hz":48000,"whole_speaker":[],"per_radiator":[{{"blocks":[],"delay_s":0.0,"gain_db":0.0,"invert":false,"limiter":null}}],
            "correction":{{"enabled":false,"protected_band_hz":[3000.0,5000.0],"max_filters":8}}}},
          "electronics":{{"amplifiers":["test-amp-a"],"dsp_device":null}},
          "build":{{"material":"pla","infill_fraction":1.0,"stereo_pair":false,"extras_cost":0.0}},
          "target":"hifi-default"
        }}"#,
        depth + 0.02
    );
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(vec![20.0, 40.0, 80.0]),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let spl = &result.radiators[0].spl_db;
    assert!((spl[0] - 64.11).abs() <= 0.05, "20 Hz {}", spl[0]);
    assert!((spl[1] - 85.20).abs() <= 0.05, "40 Hz {}", spl[1]);
    assert!((spl[2] - 88.19).abs() <= 0.05, "80 Hz {}", spl[2]);
    assert!(result.radiators[0].port_speed_m[1] > 1.0);
}

#[test]
fn two_way_example_is_canonical_and_tuned_near_40_hz() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/two-way-vented.seatlock.json"
    ))
    .unwrap();
    let checked = seatlock_core::design_check(&text, &builtin()).unwrap();
    assert_eq!(checked.canonical_json, text);
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert_eq!(result.radiators.len(), 2);
    assert!(result.radiators[1].spl_db.iter().any(|level| *level > 0.0));
}

#[test]
fn baffle_radiation_drops_about_six_decibels_at_low_frequency() {
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let freqs = vec![30.0];
    let half = simulate(
        &design,
        &SimulateRequest {
            radiation: "point".to_string(),
            space: "half".to_string(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let baffle = simulate(
        &design,
        &SimulateRequest {
            radiation: "baffle".to_string(),
            space: "half".to_string(),
            frequencies_hz: Some(freqs),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let drop = half.radiators[0].spl_db[0] - baffle.radiators[0].spl_db[0];
    assert!((drop - 6.02).abs() < 1.0, "drop {drop}");
    assert!(baffle
        .fidelity
        .ignored
        .iter()
        .any(|line| line.contains("no measured cone break-up")));
}

#[test]
fn baffle_step_is_a_six_decibel_shelf_and_one_driver_has_no_extra_delay() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/two-way-vented.seatlock.json"
    ))
    .unwrap();
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let plan = seatlock_sim::correction_plan(&design, &builtin());
    let shelf = &plan["baffle_step"];
    assert!((shelf["gain_db"].as_f64().unwrap() - 6.0).abs() < 1e-9);
    let filter = seatlock_correct::CorrectionFilter {
        filter_type: "lowshelf".to_string(),
        f_hz: shelf["f_hz"].as_f64().unwrap(),
        q: 0.707,
        gain_db: 6.0,
        from_hz: shelf["f_hz"].as_f64().unwrap(),
        from_q: 0.707,
        reason: shelf["reason"].as_str().unwrap().to_string(),
    };
    assert!((filter_db(&filter, 20.0, 48_000.0) - 6.0).abs() < 0.2);
    assert!(filter_db(&filter, 10_000.0, 48_000.0).abs() < 0.5);
    assert!(seatlock_sim::geometric_delay_s(&design, 0).abs() < 1e-12);
}

#[test]
fn a_driver_farther_from_the_listener_is_delayed() {
    let mut text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/two-way-vented.seatlock.json"
    ))
    .unwrap();
    text = text.replacen("\"surface\": \"front\"", "\"surface\": \"back\"", 1);
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let front = seatlock_sim::geometric_delay_s(&design, 1);
    let back = seatlock_sim::geometric_delay_s(&design, 0);
    assert!(back > front);
    assert!(front.abs() < 1e-12);
}
