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
    // bolts = 4 * pi * (0.0035/2)^2 * 0.008: four 3.5 mm pilot holes on the 0.132 m circle
    // solid = outer - inner - hole - bolts
    // mass = solid * 1240 kg/m^3 * infill 1
    // driver, filament, and amplifier prices are null, so the price is unknown
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let outer = 0.160 * 0.250 * 0.180;
    let air = (0.160 - 2.0 * 0.008) * (0.250 - 2.0 * 0.008) * (0.180 - 2.0 * 0.008);
    let hole = std::f64::consts::PI * (0.118_f64 / 2.0).powi(2) * 0.008;
    let bolts = 4.0 * std::f64::consts::PI * (0.0035_f64 / 2.0).powi(2) * 0.008;
    let solid = outer - air - hole - bolts;
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

#[test]
fn listen_package_exports_fir_as_mono_wav() {
    let mut raw: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    raw["dsp"]["per_radiator"][0]["blocks"] = serde_json::json!([{
        "kind": "fir",
        "taps": [0.0, 0.5, 1.0, 0.5, 0.0]
    }]);
    let design: seatlock_core::DesignFile = serde_json::from_value(raw).unwrap();
    let value = seatlock_sim::listen_package(&design, &builtin(), 48_000.0, true, 0);
    let firs = value["fir_wavs"].as_array().expect("fir_wavs");
    assert_eq!(firs.len(), 1, "{firs:?}");
    assert_eq!(firs[0]["taps"], 5);
    let b64 = firs[0]["wav_base64"].as_str().unwrap();
    let bytes = decode_base64(b64);
    assert_eq!(&bytes[0..4], b"RIFF");
    let (rate, taps) = seatlock_dsp::wav::read_mono_f32_wav(&bytes).unwrap();
    assert_eq!(rate, 48_000);
    assert_eq!(taps.len(), 5);
    assert!((taps[2] - 1.0).abs() < 1e-5);
    assert!(value["filter_text"]
        .as_str()
        .unwrap()
        .contains("Export file:"));
    let channel_firs = value["channels"][0]["firs"]
        .as_array()
        .expect("channel firs");
    assert_eq!(channel_firs.len(), 1);
    assert_eq!(channel_firs[0].as_array().unwrap().len(), 5);
    assert!(value["filter_text"]
        .as_str()
        .unwrap()
        .contains("Play convolves this FIR"));
}

#[test]
fn listen_package_expands_iir_crossover_to_biquads() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/two-way-vented.seatlock.json"
    ))
    .unwrap();
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let value = seatlock_sim::listen_package(&design, &builtin(), 48_000.0, true, 0);
    // LR4 low and high: each is two 2nd-order Butterworth → 2 biquads.
    for (i, want_band) in [(0, "low"), (1, "high")] {
        let coeffs = value["channels"][i]["coeffs"].as_array().unwrap();
        assert_eq!(coeffs.len(), 2, "radiator {i} LR4 {want_band}: {coeffs:?}");
    }
    let note = value["filter_text"].as_str().unwrap();
    assert!(note.contains("crossover linkwitz-riley"), "{note}");
}

fn decode_base64(b64: &str) -> Vec<u8> {
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in b64.bytes() {
        if c == b'=' {
            break;
        }
        let Some(v) = table.iter().position(|&t| t == c) else {
            continue;
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    out
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
fn sealed_baffle_correction_stays_near_half_space_or_names_the_limit() {
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let plan = seatlock_sim::correction_plan_with(&design, &builtin(), "baffle", "half");
    let message = plan["message"].as_str().unwrap_or("");
    let mut with_filters = design.clone();
    if let Some(filters) = plan["filters"].as_array() {
        for filter in filters {
            let part =
                serde_json::from_value::<seatlock_core::design::KindPart>(filter.clone()).unwrap();
            with_filters.dsp.per_radiator[0].blocks.push(part);
        }
    }
    let freqs = log_grid(10.0, 20_000.0, 24.0);
    let half = simulate(
        &with_filters,
        &SimulateRequest {
            radiation: "point".into(),
            space: "half".into(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let baffle = simulate(
        &with_filters,
        &SimulateRequest {
            radiation: "baffle".into(),
            space: "half".into(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let start = half.f6_hz;
    let mut worst = 0.0_f64;
    for (freq, (left, right)) in freqs
        .iter()
        .zip(half.summed_spl_db.iter().zip(&baffle.summed_spl_db))
    {
        if *freq >= start && *freq <= 200.0 {
            worst = worst.max((left - right).abs());
        }
    }
    assert!(
        worst <= 1.0 || message.contains("baffle-step"),
        "worst {worst} dB, note: {message}"
    );
}

#[test]
fn an_80_db_target_applies_a_baffle_shelf_of_at_least_3_db() {
    let mut design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    design.target = "test-80".to_string();
    let bare = simulate(
        &design,
        &SimulateRequest {
            radiation: "baffle".into(),
            space: "half".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let plan = seatlock_sim::correction_plan_with(&design, &builtin(), "baffle", "half");
    let shelf = plan["filters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|filter| filter["type"] == "lowshelf")
        .and_then(|filter| filter["gain_db"].as_f64())
        .unwrap_or(0.0);
    assert!(shelf >= 3.0, "shelf {shelf} dB, note {}", plan["message"]);
    let mut corrected = design.clone();
    for filter in plan["filters"].as_array().unwrap() {
        let part =
            serde_json::from_value::<seatlock_core::design::KindPart>(filter.clone()).unwrap();
        corrected.dsp.per_radiator[0].blocks.push(part);
    }
    let after = simulate(
        &corrected,
        &SimulateRequest {
            radiation: "baffle".into(),
            space: "half".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(
        after.flatness_db < bare.flatness_db,
        "evenness {} vs {}",
        after.flatness_db,
        bare.flatness_db
    );
}

#[test]
fn baffle_scores_for_the_page() {
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let plan = seatlock_sim::correction_plan_with(&design, &builtin(), "baffle", "half");
    let mut corrected = design.clone();
    for filter in plan["filters"].as_array().unwrap() {
        let part =
            serde_json::from_value::<seatlock_core::design::KindPart>(filter.clone()).unwrap();
        corrected.dsp.per_radiator[0].blocks.push(part);
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
        &corrected,
        &SimulateRequest {
            radiation: "baffle".into(),
            space: "half".into(),
            frequencies_hz: Some(freqs),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert_eq!(format!("{:.0}", result.f6_hz), "113");
    assert_eq!(format!("{:.1}", result.flatness_db), "7.4");
    assert_eq!(format!("{:.0}", result.max_spl_metric_db), "83");
}

#[test]
fn level_2_outer_shapes_register() {
    let registry = builtin();
    let mut fields = serde_json::Map::new();
    fields.insert("width_m".into(), serde_json::json!(0.2));
    fields.insert("height_m".into(), serde_json::json!(0.3));
    fields.insert("depth_m".into(), serde_json::json!(0.2));
    fields.insert("wall_m".into(), serde_json::json!(0.012));
    fields.insert("edge_radius_m".into(), serde_json::json!(0.008));
    let rounded = registry.create_outer("rounded", &fields).unwrap();
    assert!(rounded.interior_volume_m3().unwrap() > 0.005);
    fields.insert("chamfer_m".into(), serde_json::json!(0.005));
    assert!(registry.create_outer("chamfered", &fields).is_ok());
    let mut sloped = serde_json::Map::new();
    sloped.insert("width_m".into(), serde_json::json!(0.2));
    sloped.insert("height_m".into(), serde_json::json!(0.3));
    sloped.insert("depth_bottom_m".into(), serde_json::json!(0.2));
    sloped.insert("depth_top_m".into(), serde_json::json!(0.12));
    sloped.insert("wall_m".into(), serde_json::json!(0.012));
    assert!(
        registry
            .create_outer("sloped", &sloped)
            .unwrap()
            .interior_volume_m3()
            .unwrap()
            > 0.0
    );
    let mut prism = serde_json::Map::new();
    prism.insert(
        "outline_m".into(),
        serde_json::json!([[0.0, 0.0], [0.2, 0.0], [0.1, 0.2]]),
    );
    prism.insert("length_m".into(), serde_json::json!(0.25));
    prism.insert("wall_m".into(), serde_json::json!(0.012));
    assert!(
        registry
            .create_outer("prism", &prism)
            .unwrap()
            .interior_volume_m3()
            .unwrap()
            > 0.0
    );
}

#[test]
fn a_pvc_tube_buys_the_whole_pipe_and_names_the_offcut() {
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    let outer = value
        .pointer_mut("/speaker/enclosure/outer")
        .unwrap()
        .as_object_mut()
        .unwrap();
    outer.clear();
    outer.insert("kind".into(), serde_json::json!("tube"));
    outer.insert(
        "stock_id".into(),
        serde_json::json!("charlotte-pvc-sch40-4in"),
    );
    outer.insert("length_m".into(), serde_json::json!(0.5));
    // The driver sits at the middle of the 0.1143 m end cap.
    *value
        .pointer_mut("/speaker/enclosure/mounts/0/at_m")
        .unwrap() = serde_json::json!([0.05715, 0.05715]);
    let design = serde_json::from_value::<seatlock_core::DesignFile>(value).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let radius = 0.1022604 / 2.0;
    let expected = std::f64::consts::PI * radius * radius * 0.5;
    assert!(
        (report.interior_volume_m3 - expected).abs() < 1e-9,
        "{}",
        report.interior_volume_m3
    );
    assert!(!report.user_wall);
    assert!(report.cut_list.iter().any(|line| line.contains("0.500")));
    assert!(report.cut_list.iter().any(|line| line.contains("offcut")));
    let pipe = report
        .cost
        .lines
        .iter()
        .find(|line| line.item.contains("PVC"))
        .unwrap();
    assert_eq!(pipe.unit_usd, Some(77.13));
}

#[test]
fn a_form_tube_wall_is_the_users_own_number() {
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    let outer = value
        .pointer_mut("/speaker/enclosure/outer")
        .unwrap()
        .as_object_mut()
        .unwrap();
    outer.clear();
    outer.insert("kind".into(), serde_json::json!("tube"));
    outer.insert(
        "stock_id".into(),
        serde_json::json!("sakrete-form-tube-8in"),
    );
    outer.insert("length_m".into(), serde_json::json!(1.0));
    outer.insert("wall_m".into(), serde_json::json!(0.003));
    let design = serde_json::from_value::<seatlock_core::DesignFile>(value).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(report.user_wall);
    assert!(report
        .warnings
        .iter()
        .any(|line| line.contains("user's own")));
    let inside = 0.2032_f64 - 2.0 * 0.003;
    let radius = inside / 2.0;
    let expected = std::f64::consts::PI * radius * radius;
    assert!((report.interior_volume_m3 - expected).abs() < 1e-9);
}

#[test]
fn nd25_simulates_without_a_cone_travel_limit() {
    let mut text = read_example();
    text = text.replace("test-woofer-a", "dayton-nd25fa-4");
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(!result.radiators.is_empty());
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.message.contains("cone travel")));
    assert!(result.radiators[0].spl_db.iter().any(|level| *level > 0.0));
}

#[test]
fn nd25_on_the_front_exports_from_the_sheet_drawing() {
    let mut text = read_example();
    text = text.replace("test-woofer-a", "dayton-nd25fa-4");
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&text).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(
        report
            .warnings
            .iter()
            .all(|line| !line.contains("mounting size not published")),
        "{:?}",
        report.warnings
    );
    assert!(report.can_export_print, "{:?}", report.warnings);
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

fn tube_design(length_m: f64, stereo: bool, driver: &str, amp: &str) -> seatlock_core::DesignFile {
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    *value.pointer_mut("/speaker/enclosure/outer").unwrap() = serde_json::json!({
        "kind": "tube", "stock_id": "charlotte-pvc-sch40-4in",
        "length_m": length_m, "cap_thickness_m": 0.008
    });
    *value
        .pointer_mut("/speaker/enclosure/mounts/0/at_m")
        .unwrap() = serde_json::json!([0.05715, 0.05715]);
    *value.pointer_mut("/speaker/radiators/0/driver_id").unwrap() = serde_json::json!(driver);
    *value.pointer_mut("/electronics/amplifiers").unwrap() = serde_json::json!([amp]);
    *value.pointer_mut("/build/stereo_pair").unwrap() = serde_json::json!(stereo);
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_stereo_pair_of_tubes_buys_pipe_for_every_cut() {
    // Two 1.6 m cuts do not fit one 3.048 m pipe, so the pair needs two pieces.
    let design = tube_design(1.6, true, "test-woofer-a", "test-amp-a");
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let pipe = report
        .cost
        .lines
        .iter()
        .find(|line| line.item.contains("PVC"))
        .expect("a pipe line");
    assert_eq!(pipe.quantity, 2.0);
    assert_eq!(report.cost.cut_list.len(), 2, "{:?}", report.cost.cut_list);
    // Two 0.5 m cuts share one piece.
    let design = tube_design(0.5, true, "test-woofer-a", "test-amp-a");
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let pipe = report
        .cost
        .lines
        .iter()
        .find(|line| line.item.contains("PVC"))
        .unwrap();
    assert_eq!(pipe.quantity, 1.0);
    assert!(
        report.cost.cut_list[0].contains("2.048"),
        "{:?}",
        report.cost.cut_list
    );
}

#[test]
fn a_tube_cut_longer_than_the_piece_is_an_error() {
    let design = tube_design(3.2, false, "test-woofer-a", "test-amp-a");
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(!report.cost.complete);
    assert!(
        report
            .cost
            .problems
            .iter()
            .any(|line| line.contains("longer than")),
        "{:?}",
        report.cost.problems
    );
}

#[test]
fn chk_cost_1_tube_hand_calculation() {
    // A stereo pair: TCP115-4 in a 0.5 m cut of 4 in. Schedule 40 PVC with two printed PLA caps.
    // drivers   2 x $15.98
    // pipe      both 0.5 m cuts fit one 3.048 m piece: 1 x $77.13
    // caps      per speaker: 2 discs of the 0.1143 m outside diameter, 0.008 m thick,
    //           minus the 0.095758 m cut-out and four 3.5 mm bolt holes in the front cap;
    //           PLA 1240 kg/m^3 at $13.49/kg
    // amplifier 2 channels needed, one KABD-4100 has 4: 1 x $89.98
    let design = tube_design(0.5, true, "dayton-tcp115-4", "dayton-kabd-4100");
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let pi = std::f64::consts::PI;
    let disc = pi * (0.1143_f64 / 2.0).powi(2) * 0.008;
    let hole = pi * (0.095758_f64 / 2.0).powi(2) * 0.008;
    let bolts = 4.0 * pi * (0.0035_f64 / 2.0).powi(2) * 0.008;
    let caps_kg = (2.0 * disc - hole - bolts) * 1240.0 * 2.0;
    let expected = 2.0 * 15.98 + 77.13 + caps_kg * 13.49 + 89.98;
    assert!(report.cost.complete, "{:?}", report.cost.problems);
    assert!(!report.cost.unconfirmed);
    assert!(
        (report.cost.known_total_usd - expected).abs() < 1e-6,
        "{} against {}",
        report.cost.known_total_usd,
        expected
    );
    assert!((report.cost.print_mass_kg - caps_kg).abs() < 1e-9);
}

#[test]
fn chk_cost_1_sealed_test_dollars_and_print_time() {
    // sealed-test is a stereo pair. Its test driver and test amplifier have no price,
    // so the total is a lower bound: the PLA for two cabinets.
    let design = serde_json::from_str::<seatlock_core::DesignFile>(&read_example()).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let outer = 0.160 * 0.250 * 0.180;
    let air = (0.160 - 2.0 * 0.008) * (0.250 - 2.0 * 0.008) * (0.180 - 2.0 * 0.008);
    let hole = std::f64::consts::PI * (0.118_f64 / 2.0).powi(2) * 0.008;
    let bolts = 4.0 * std::f64::consts::PI * (0.0035_f64 / 2.0).powi(2) * 0.008;
    let solid = outer - air - hole - bolts;
    let pla = solid * 1240.0 * 2.0 * 13.49;
    assert!(!report.cost.complete);
    assert!(
        (report.cost.known_total_usd - pla).abs() < 1e-9,
        "{}",
        report.cost.known_total_usd
    );
    // Print time from the X2D's 40 mm^3/s maximum flow is a lower bound.
    let hours = solid * 2.0 / 4.0e-8 / 3600.0;
    let shown = report.cost.print_time_h_at_least.unwrap();
    assert!((shown - hours).abs() < 1e-9, "{shown}");
    assert!(
        report.print_note.contains("256 x 256 x 260"),
        "{}",
        report.print_note
    );
}

#[test]
fn a_tube_gets_the_diffraction_of_its_round_end_cap() {
    // A source at the middle of the 0.1143 m end cap. On axis, `baffle` divided by half space
    // is the edge-diffraction factor of that circle. Near k*R = pi (about 3 kHz) a centred
    // source on a round baffle has its strongest peak, which a rectangle does not have.
    let design = tube_design(0.5, false, "test-woofer-a", "test-amp-a");
    let freqs = vec![200.0, 1000.0, 3000.0, 5000.0];
    let request = |radiation: &str| SimulateRequest {
        radiation: radiation.into(),
        space: "half".into(),
        frequencies_hz: Some(freqs.clone()),
        ..SimulateRequest::default()
    };
    let baffle = simulate(&design, &request("baffle"), &builtin());
    let half = simulate(&design, &request("point"), &builtin());
    assert!(
        baffle
            .issues
            .iter()
            .all(|issue| issue.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        baffle.issues
    );
    let d = 0.1143;
    let center = [d / 2.0, 0.0, d / 2.0];
    let listener = [d / 2.0, -1.0, d / 2.0];
    for (bin, freq) in freqs.iter().enumerate() {
        let outline = seatlock_acoustics::circular_outline(center, d / 2.0, [0.0, -1.0, 0.0], 256);
        let factor = seatlock_acoustics::diffraction_factor(center, listener, &outline, *freq);
        let expected = 20.0 * factor.norm().log10();
        let got = baffle.summed_spl_db[bin] - half.summed_spl_db[bin];
        assert!(
            (got - expected).abs() < 0.1,
            "{freq} Hz: got {got:.2} dB, expected {expected:.2} dB"
        );
    }
}

/// A shape that answers only the air volume, as a half-written new member would.
struct HalfWritten;

impl seatlock_core::registry::OuterShape for HalfWritten {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(0.005)
    }
    fn baffle_width_m(&self) -> f64 {
        0.16
    }
}

#[test]
fn an_unfinished_shape_says_so_and_gives_no_curves() {
    let mut registry = builtin();
    registry.register_outer("half_written", |_| Ok(Box::new(HalfWritten)));
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    *value.pointer_mut("/speaker/enclosure/outer").unwrap() =
        serde_json::json!({ "kind": "half_written" });
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &registry);
    assert!(result.radiators.is_empty(), "gave curves");
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.message.contains("not finished yet")),
        "{:?}",
        result.issues
    );
}

#[test]
fn the_level_2_shapes_simulate_with_baffle_radiation_from_their_own_front() {
    // Each shape's front is where sealed-test's driver sits, and the baffle model takes edge
    // diffraction from that shape's own outline, so each gives its own low-frequency step.
    let shapes = [
        serde_json::json!({
            "kind": "rounded", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18,
            "wall_m": 0.008, "edge_radius_m": 0.02
        }),
        serde_json::json!({
            "kind": "chamfered", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18,
            "wall_m": 0.008, "chamfer_m": 0.01
        }),
        serde_json::json!({
            "kind": "sloped", "width_m": 0.16, "height_m": 0.25, "depth_bottom_m": 0.20,
            "depth_top_m": 0.15, "wall_m": 0.008
        }),
        serde_json::json!({
            "kind": "prism", "length_m": 0.18, "wall_m": 0.008,
            "outline_m": [[0.0, 0.0], [0.16, 0.0], [0.16, 0.20], [0.08, 0.25], [0.0, 0.20]]
        }),
    ];
    for outer in shapes {
        let kind = outer["kind"].as_str().unwrap().to_string();
        let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
        *value.pointer_mut("/speaker/enclosure/outer").unwrap() = outer;
        let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
        let request = SimulateRequest {
            radiation: "baffle".into(),
            space: "half".into(),
            ..SimulateRequest::default()
        };
        let result = simulate(&design, &request, &builtin());
        assert!(!result.radiators.is_empty(), "{kind}: {:?}", result.issues);
        assert!(
            !result
                .fidelity
                .ignored
                .iter()
                .any(|line| line.contains("treated as a point source")),
            "{kind}: {:?}",
            result.fidelity.ignored
        );
        let report =
            seatlock_sim::geometry_report(&design).unwrap_or_else(|err| panic!("{kind}: {err}"));
        assert!(report.solid_volume_m3 > 0.0, "{kind}");
    }
}

#[test]
fn chk_cost_1_sheet_box_hand_calculation() {
    // sealed-test's outside size cut from 3/4 in. MDF: nothing printed, a cut list, whole sheets.
    // Per speaker: front and back 0.16 x 0.25; left and right (0.18 - 2t) x 0.25;
    // top and bottom (0.16 - 2t) x (0.18 - 2t), with t = 0.01905 m.
    // A stereo pair's twelve panels fit one 1.2192 x 2.4384 m sheet: 1 x $49.98.
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    *value.pointer_mut("/speaker/enclosure/outer").unwrap() = serde_json::json!({
        "kind": "sheet_box", "stock_id": "ultrastock-mdf-3-4",
        "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18
    });
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(!result.radiators.is_empty(), "{:?}", result.issues);
    let t = 0.01905;
    let air = (0.16 - 2.0 * t) * (0.25 - 2.0 * t) * (0.18 - 2.0 * t);
    assert!((result.interior_volume_m3.unwrap() - air).abs() < 1e-9);
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert_eq!(report.solid_volume_m3, 0.0);
    assert_eq!(report.cost.print_mass_kg, 0.0);
    let sheet = report
        .cost
        .lines
        .iter()
        .find(|line| line.item.contains("MDF"))
        .unwrap();
    assert_eq!(sheet.quantity, 1.0);
    assert_eq!(sheet.total_usd, Some(49.98));
    assert!(
        sheet.unconfirmed,
        "the MDF price came from a category listing"
    );
    let panels: usize = report
        .cost
        .cut_list
        .iter()
        .map(|line| line.matches(" mm").count())
        .sum();
    assert_eq!(panels, 12, "{:?}", report.cost.cut_list);
}

#[test]
fn chk_geo_1_every_example_design_simulates_and_its_voxels_agree() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../designs/examples");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let design: seatlock_core::DesignFile = serde_json::from_str(&text).unwrap();
        let name = path.display();
        let result = simulate(&design, &SimulateRequest::default(), &builtin());
        assert!(!result.radiators.is_empty(), "{name}: {:?}", result.issues);
        let report =
            seatlock_sim::geometry_report(&design).unwrap_or_else(|err| panic!("{name}: {err}"));
        let air = report.interior_volume_m3;
        assert!(
            (report.voxel_air_volume_m3 - air).abs() / air < 0.02,
            "{name}: formula {air:.6e}, voxels {:.6e}",
            report.voxel_air_volume_m3
        );
        count += 1;
    }
    assert!(count >= 8, "{count} examples");
}

fn sealed_with_mount(edit: impl Fn(&mut serde_json::Value)) -> seatlock_core::DesignFile {
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    edit(value.pointer_mut("/speaker/enclosure/mounts/0").unwrap());
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_recess_takes_the_flange_ring_out_and_shortens_the_bolt_holes() {
    // test-woofer-a: flange 0.146 m, cut-out 0.118 m, four bolts on 0.132 m. Recess 3 mm.
    let plain = seatlock_sim::geometry_report(&sealed_with_mount(|_| {})).unwrap();
    let design = sealed_with_mount(|mount| mount["recess_m"] = serde_json::json!(0.003));
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let pi = std::f64::consts::PI;
    let ring = pi * ((0.146_f64 / 2.0).powi(2) - (0.118_f64 / 2.0).powi(2)) * 0.003;
    let bolts_shorter = 4.0 * pi * (0.0035_f64 / 2.0).powi(2) * 0.003;
    let expected = plain.solid_volume_m3 - ring + bolts_shorter;
    assert!(
        (report.solid_volume_m3 - expected).abs() < 1e-12,
        "{} against {expected}",
        report.solid_volume_m3
    );
    assert!(report.can_export_print, "{:?}", report.warnings);
    let tree = report.print_tree.to_string();
    assert!(tree.contains("0.073"), "no flange-sized recess: {tree}");
}

#[test]
fn bolt_holes_sit_on_the_bolt_circle_in_the_face() {
    let report = seatlock_sim::geometry_report(&sealed_with_mount(|_| {})).unwrap();
    // sealed-test's mount is at x 0.08, z 0.15 on the front (y = 0).
    let mut found = 0;
    fn walk(node: &serde_json::Value, found: &mut usize) {
        if node["op"] == "cylinder" && (node["radius_m"].as_f64().unwrap() - 0.00175).abs() < 1e-12
        {
            let base = node["base_m"].as_array().unwrap();
            let (x, z) = (base[0].as_f64().unwrap(), base[2].as_f64().unwrap());
            let r = ((x - 0.08).powi(2) + (z - 0.15).powi(2)).sqrt();
            assert!((r - 0.066).abs() < 1e-9, "bolt at radius {r}");
            assert!(node["base_m"][1].as_f64().unwrap() < 0.0);
            *found += 1;
        }
        if let Some(children) = node["of"].as_array() {
            for child in children {
                walk(child, found);
            }
        }
    }
    walk(&report.print_tree, &mut found);
    assert_eq!(found, 4);
}

#[test]
fn a_missing_bolt_circle_leaves_the_screw_holes_out_and_a_bad_recess_refuses_the_export() {
    // dayton-nd65-4 prints no bolt circle; a recess must be thinner than the wall.
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    value["speaker"]["radiators"][0]["driver_id"] = serde_json::json!("dayton-nd65-4");
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    // No bolt circle: the hole is cut, the screw holes are left to the builder, and the
    // report says so (owner, 2026-11-12: a missing detail must not stop a design).
    assert!(report.can_export_print);
    assert!(
        report
            .warnings
            .iter()
            .any(|line| line.contains("bolt circle") && line.contains("mark the holes")),
        "{:?}",
        report.warnings
    );
    let deep = sealed_with_mount(|mount| mount["recess_m"] = serde_json::json!(0.008));
    let report = seatlock_sim::geometry_report(&deep).unwrap();
    assert!(!report.can_export_print);
    assert!(
        report.warnings.iter().any(|line| line.contains("recess")),
        "{:?}",
        report.warnings
    );
}

fn two_way() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../designs/examples/two-way-vented.seatlock.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn a_straight_round_port_is_bored_through_the_front_and_printed_as_a_tube() {
    // two-way-vented: 40 mm port, 87.98 mm from inside the box to the front face, 12 mm wall,
    // 3 mm tube wall. The tube adds its ring over the 75.98 mm inside; the bore through the
    // wall takes out a 40 mm disc 12 mm deep.
    let design: seatlock_core::DesignFile = serde_json::from_value(two_way()).unwrap();
    let mut sealed = two_way();
    sealed["speaker"]["enclosure"]["ducts"] = serde_json::json!([]);
    let sealed: seatlock_core::DesignFile = serde_json::from_value(sealed).unwrap();
    let with_port = seatlock_sim::geometry_report(&design).unwrap();
    let without = seatlock_sim::geometry_report(&sealed).unwrap();
    let pi = std::f64::consts::PI;
    let (r, t, wall, length) = (0.02_f64, 0.003_f64, 0.012_f64, 0.08798_f64);
    let inside = length - wall;
    let added = pi * ((r + t).powi(2) - r * r) * inside - pi * r * r * wall;
    let got = with_port.solid_volume_m3 - without.solid_volume_m3;
    assert!((got - added).abs() < 1e-12, "{got} against {added}");
    assert!(with_port.can_export_print, "{:?}", with_port.warnings);

    // The tube's outside takes air from the box.
    let request = SimulateRequest::default();
    let vented = simulate(&design, &request, &builtin());
    let closed = simulate(&sealed, &request, &builtin());
    let displaced = pi * (r + t).powi(2) * inside;
    let lost = closed.interior_volume_m3.unwrap() - vented.interior_volume_m3.unwrap();
    assert!(
        (lost - displaced).abs() < 1e-12,
        "{lost} against {displaced}"
    );
}

#[test]
fn a_port_that_cannot_be_printed_is_simulated_and_refuses_the_export() {
    let cases = [
        // Its path stops inside the box, short of the front face.
        (
            serde_json::json!([[0.045, 0.2, 0.045], [0.045, 0.11202, 0.045]]),
            "not on the front",
        ),
        // Its opening sits inside the woofer's flange.
        (
            serde_json::json!([[0.137, 0.08798, 0.08], [0.137, 0.0, 0.08]]),
            "woofer",
        ),
        // A path that doubles back on itself (a bend is printed since Level 3).
        (
            serde_json::json!([
                [0.045, 0.1, 0.1],
                [0.045, 0.1, 0.045],
                [0.045, 0.1, 0.09],
                [0.045, 0.0, 0.09]
            ]),
            "doubles straight back",
        ),
    ];
    for (path, words) in cases {
        let mut value = two_way();
        value["speaker"]["enclosure"]["ducts"][0]["path_m"] = path;
        let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
        let report = seatlock_sim::geometry_report(&design).unwrap();
        assert!(!report.can_export_print, "{words}");
        assert!(
            report.warnings.iter().any(|line| line.contains(words)),
            "{words}: {:?}",
            report.warnings
        );
        let result = simulate(&design, &SimulateRequest::default(), &builtin());
        assert!(!result.radiators.is_empty(), "{words}");
    }
}

#[test]
fn a_slot_port_is_printed_with_its_own_size() {
    let mut value = two_way();
    value["speaker"]["enclosure"]["ducts"][0]["section"] =
        serde_json::json!({ "shape": "rect", "start_m": [0.05, 0.02], "end_m": [0.05, 0.02] });
    value["speaker"]["enclosure"]["ducts"][0]["path_m"] =
        serde_json::json!([[0.045, 0.08798, 0.045], [0.045, 0.0, 0.045]]);
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(report.can_export_print, "{:?}", report.warnings);
    assert!(report.print_tree.to_string().contains("\"op\":\"prism\""));
}

fn example(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn one_chamber_that_fills_the_inside_changes_nothing() {
    // sealed-test with its whole inside named as one chamber: no partition, same air, same
    // sound.
    let plain: seatlock_core::DesignFile = serde_json::from_str(&read_example()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&read_example()).unwrap();
    value["speaker"]["enclosure"]["chambers"] = serde_json::json!([
        { "id": "main", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242] }
    ]);
    let named: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let request = SimulateRequest::default();
    let a = simulate(&plain, &request, &builtin());
    let b = simulate(&named, &request, &builtin());
    assert!(!b.radiators.is_empty(), "{:?}", b.issues);
    for (x, y) in a.summed_spl_db.iter().zip(&b.summed_spl_db) {
        assert!((x - y).abs() < 1e-9, "{x} {y}");
    }
    let ra = seatlock_sim::geometry_report(&plain).unwrap();
    let rb = seatlock_sim::geometry_report(&named).unwrap();
    assert!((ra.solid_volume_m3 - rb.solid_volume_m3).abs() < 1e-12);
    assert!(rb.can_export_print, "{:?}", rb.warnings);
}

#[test]
fn two_chambers_print_a_partition_and_vent_everything_at_low_frequency() {
    // two-chamber: front 0.25 x 0.138 x 0.416 m and rear 0.25 x 0.126 x 0.416 m inside a box
    // whose inside is 0.25 x 0.276 x 0.416 m, so the partition is 0.25 x 0.012 x 0.416 m.
    let design: seatlock_core::DesignFile = serde_json::from_value(example("two-chamber")).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(report.can_export_print, "{:?}", report.warnings);
    let front = 0.25 * 0.138 * 0.416;
    let rear = 0.25 * 0.126 * 0.416;
    assert!((report.interior_volume_m3 - (front + rear)).abs() < 1e-12);
    let mut open = example("two-chamber");
    open["speaker"]["enclosure"]["chambers"] = serde_json::json!([]);
    open["speaker"]["enclosure"]["ducts"] = serde_json::json!([]);
    for mount in open["speaker"]["enclosure"]["mounts"]
        .as_array_mut()
        .unwrap()
    {
        mount["rear_into"] = serde_json::json!("main");
    }
    let open: seatlock_core::DesignFile = serde_json::from_value(open).unwrap();
    let plain = seatlock_sim::geometry_report(&open).unwrap();
    // The coupler: a 40 mm tube, 3 mm wall, 50 mm long; 12 mm of it is in the partition.
    // The port: 80 mm long, 12 mm through the back wall, the rest in the rear chamber.
    let pi = std::f64::consts::PI;
    let ring = pi * (0.023_f64.powi(2) - 0.02_f64.powi(2));
    let disc = pi * 0.02_f64.powi(2);
    let coupler = ring * 0.038 - disc * 0.012;
    let port = ring * 0.068 - disc * 0.012;
    let partition = 0.25 * 0.012 * 0.416;
    let added = report.solid_volume_m3 - plain.solid_volume_m3;
    let expected = partition + coupler + port;
    assert!(
        (added - expected).abs() < 1e-12,
        "{added} against {expected}"
    );

    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(!result.radiators.is_empty(), "{:?}", result.issues);
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|line| line.contains("Several chambers")));
    // Net air: each chamber less its tube lengths and, at the front, the woofer behind it.
    let tubes = pi * 0.023_f64.powi(2) * (0.038 + 0.068);
    let net = result.interior_volume_m3.unwrap();
    assert!(
        net < front + rear - tubes && net > front + rear - tubes - 0.002,
        "{net}"
    );
}

#[test]
fn chambers_that_do_not_fit_are_refused_with_the_reason() {
    type Edit<'a> = &'a dyn Fn(&mut serde_json::Value);
    let cases: [(Edit, &str); 3] = [
        (
            &|v| v["speaker"]["enclosure"]["chambers"][1]["min_m"][1] = serde_json::json!(0.14),
            "overlap",
        ),
        (
            &|v| v["speaker"]["enclosure"]["chambers"][1]["max_m"][1] = serde_json::json!(0.295),
            "outside the cabinet",
        ),
        (
            &|v| v["speaker"]["enclosure"]["mounts"][0]["rear_into"] = serde_json::json!("attic"),
            "not one of the chambers",
        ),
    ];
    for (edit, words) in cases {
        let mut value = example("two-chamber");
        edit(&mut value);
        let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
        let result = simulate(&design, &SimulateRequest::default(), &builtin());
        assert!(result.radiators.is_empty(), "{words}");
        assert!(
            result
                .issues
                .iter()
                .any(|issue| issue.message.contains(words)),
            "{words}: {:?}",
            result.issues
        );
    }
    // A driver whose hole lands on the partition cannot be printed.
    let mut value = example("two-chamber");
    value["speaker"]["enclosure"]["mounts"][0]["surface"] = serde_json::json!("left");
    value["speaker"]["enclosure"]["mounts"][0]["at_m"] = serde_json::json!([0.144, 0.25]);
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(!report.can_export_print);
    assert!(report
        .warnings
        .iter()
        .any(|line| line.contains("does not open into chamber front")));
}

#[test]
fn a_cabinet_that_fits_the_printer_is_not_split() {
    let design: seatlock_core::DesignFile = serde_json::from_str(&read_example()).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert!(report.print_split.parts.is_empty());
    assert!(report.print_split.pins.is_empty());
    assert!(report.print_note.contains("fits"), "{}", report.print_note);
}

#[test]
fn the_fewest_parts_win_whichever_way_the_cabinet_stands() {
    let bed = [0.256, 0.256, 0.260];
    // A 0.30 m side is longer than every side of the bed, so it is cut once whichever way.
    assert_eq!(
        seatlock_sim::split::pieces([0.25, 0.20, 0.30], bed),
        [1, 1, 2]
    );
    // 0.259 m tall fits standing on the 0.260 m axis.
    assert_eq!(
        seatlock_sim::split::pieces([0.20, 0.20, 0.259], bed),
        [1, 1, 1]
    );
    // 0.259 m deep fits too, by standing the cabinet on its back.
    assert_eq!(
        seatlock_sim::split::pieces([0.20, 0.259, 0.20], bed),
        [1, 1, 1]
    );
    assert_eq!(
        seatlock_sim::split::pieces([0.274, 0.30, 0.44], bed),
        [2, 2, 2]
    );
}

#[test]
fn a_split_cabinet_has_pins_in_plastic_across_every_cut_and_parts_that_fit() {
    // two-chamber is 0.274 x 0.30 x 0.44 m: no side fits the 256 x 256 x 260 mm bed, so it
    // is cut in half along each axis into 8 parts, with 12 mm walls to hold 4.2 mm pin holes.
    let design: seatlock_core::DesignFile = serde_json::from_value(example("two-chamber")).unwrap();
    let report = seatlock_sim::geometry_report(&design).unwrap();
    let split = &report.print_split;
    assert_eq!(split.parts.len(), 8);
    for part in &split.parts {
        let mut size: Vec<f64> = (0..3)
            .map(|i| part.max_m[i] - part.min_m[i] - 0.001)
            .collect();
        size.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            size[0] <= 0.256 && size[1] <= 0.256 && size[2] <= 0.260,
            "{size:?}"
        );
    }
    assert!(split.pins.len() >= 24, "{} pins", split.pins.len());
    // Every pin sits on a cut plane, in plastic, away from the drivers' flanges.
    for pin in &split.pins {
        let half = [0.137, 0.15, 0.22][pin.axis];
        assert!((pin.center_m[pin.axis] - half).abs() < 1e-12, "{pin:?}");
    }
    let pi = std::f64::consts::PI;
    let hole = pi * 0.0021_f64.powi(2) * 0.016;
    assert!((split.pin_holes_m3 - split.pins.len() as f64 * hole).abs() < 1e-15);
    // The pins are printed too, so the plastic bought includes them.
    let density = 1240.0;
    let mass = (report.solid_volume_m3 + split.pins_m3) * density;
    assert!((report.print_mass_kg - mass).abs() < 1e-12);
    assert!(
        report.print_note.contains("8 parts"),
        "{}",
        report.print_note
    );
}
