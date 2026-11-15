//! Passive crossover networks in the simulator (README section 7).

use seatlock_sim::{builtin, listen_package, simulate, SimulateRequest};
use serde_json::{json, Value};

fn example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn design(value: &Value) -> seatlock_core::DesignFile {
    serde_json::from_value(value.clone()).unwrap()
}

fn level_at(value: &Value, freq: f64) -> f64 {
    let result = simulate(
        &design(value),
        &SimulateRequest {
            frequencies_hz: Some(vec![freq]),
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
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    result.summed_spl_db[0]
}

/// A driver wired straight across the amplifier matches the same design with no digital
/// filters (empty per-radiator blocks), within a tiny numerical gap.
#[test]
fn a_direct_passive_wire_matches_no_digital_filter() {
    let mut wired = example("sealed-test");
    wired["dsp"]["per_radiator"][0]["blocks"] = json!([]);
    wired["dsp"]["passive"] = json!({
        "source": ["amp", "gnd"],
        "elements": [
            { "kind": "driver", "a": "amp", "b": "gnd", "radiator": 0 }
        ]
    });
    let mut plain = example("sealed-test");
    plain["dsp"]["per_radiator"][0]["blocks"] = json!([]);
    for freq in [50.0, 200.0, 1000.0, 4000.0] {
        let a = level_at(&wired, freq);
        let b = level_at(&plain, freq);
        assert!(
            (a - b).abs() < 0.02,
            "{freq} Hz: passive wire {a} against plain {b}"
        );
    }
    let result = simulate(
        &design(&wired),
        &SimulateRequest {
            radiation: "point".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|l| l.starts_with("Passive crossover")));
}

/// A first-order LR low-pass into the woofer and CR high-pass into the tweeter cross so the
/// woofer dominates below the corner and the tweeter above.
#[test]
fn a_passive_two_way_splits_bass_and_treble() {
    let mut value = example("two-way-vented");
    // Resistive corner ~2.5 kHz on ~6 ohm loads (order-of-magnitude; real Z varies).
    let fc = 2500.0;
    let r = 6.0;
    let l = r / (2.0 * std::f64::consts::PI * fc);
    let c = 1.0 / (2.0 * std::f64::consts::PI * fc * r);
    value["dsp"]["passive"] = json!({
        "source": ["amp", "gnd"],
        "elements": [
            { "kind": "L", "a": "amp", "b": "w", "henry": l },
            { "kind": "driver", "a": "w", "b": "gnd", "radiator": 0 },
            { "kind": "C", "a": "amp", "b": "t", "farad": c },
            { "kind": "driver", "a": "t", "b": "gnd", "radiator": 1 }
        ]
    });
    // Clear digital crossovers so the note about unused blocks is honest if any remain.
    value["dsp"]["per_radiator"][0]["blocks"] = json!([]);
    value["dsp"]["per_radiator"][1]["blocks"] = json!([]);
    let result = simulate(
        &design(&value),
        &SimulateRequest {
            frequencies_hz: Some(vec![200.0, 8000.0]),
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
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|l| l.starts_with("Passive crossover")));
    let woofer_low = result.radiators[0].spl_db[0];
    let tweeter_low = result.radiators[1].spl_db[0];
    let woofer_high = result.radiators[0].spl_db[1];
    let tweeter_high = result.radiators[1].spl_db[1];
    println!(
        "passive two-way: 200 Hz woofer {woofer_low:.1} / tweeter {tweeter_low:.1}; \
         8 kHz woofer {woofer_high:.1} / tweeter {tweeter_high:.1}"
    );
    assert!(
        woofer_low > tweeter_low + 10.0,
        "woofer should dominate at 200 Hz: {woofer_low} vs {tweeter_low}"
    );
    assert!(
        tweeter_high > woofer_high + 6.0,
        "tweeter should dominate at 8 kHz: {tweeter_high} vs {woofer_high}"
    );
}

/// First-order two-way: listen impulses carry the passive split, and the worklet channels
/// leave digital crossover blocks out (they stay in the convolver path instead).
#[test]
fn listen_impulses_carry_the_passive_network() {
    let mut value = example("two-way-vented");
    let fc = 2500.0;
    let r = 6.0;
    let l = r / (2.0 * std::f64::consts::PI * fc);
    let c = 1.0 / (2.0 * std::f64::consts::PI * fc * r);
    // Leftover digital blocks must not be played when passive is set.
    value["dsp"]["per_radiator"][0]["blocks"] = json!([
        { "kind": "biquad", "type": "lowpass", "f_hz": 500.0, "q": 0.707 }
    ]);
    value["dsp"]["per_radiator"][1]["blocks"] = json!([
        { "kind": "biquad", "type": "highpass", "f_hz": 8000.0, "q": 0.707 }
    ]);
    value["dsp"]["passive"] = json!({
        "source": ["amp", "gnd"],
        "elements": [
            { "kind": "L", "a": "amp", "b": "w", "henry": l },
            { "kind": "driver", "a": "w", "b": "gnd", "radiator": 0 },
            { "kind": "C", "a": "amp", "b": "t", "farad": c },
            { "kind": "driver", "a": "t", "b": "gnd", "radiator": 1 }
        ]
    });
    let design = design(&value);
    let pack = listen_package(&design, &builtin(), 48_000.0, false, 2048);
    let text = pack["filter_text"].as_str().unwrap();
    assert!(
        text.contains("Passive crossover"),
        "listen package should name the passive path: {text}"
    );
    assert!(
        text.contains("frozen"),
        "listen package should say loads are frozen: {text}"
    );
    assert!(
        text.contains("left out of playback"),
        "leftover digital blocks should be noted: {text}"
    );
    for index in 0..2 {
        let coeffs = pack["channels"][index]["coeffs"].as_array().unwrap();
        assert!(
            coeffs.is_empty(),
            "radiator {index} must not play digital biquads when passive is set: {coeffs:?}"
        );
    }
    let woofer = pack["impulses"][0].as_array().unwrap();
    let tweeter = pack["impulses"][1].as_array().unwrap();
    assert_eq!(woofer.len(), 2048);
    assert_eq!(tweeter.len(), 2048);
    let woofer_ir: Vec<f64> = woofer.iter().map(|v| v.as_f64().unwrap()).collect();
    let tweeter_ir: Vec<f64> = tweeter.iter().map(|v| v.as_f64().unwrap()).collect();
    let woofer_low = tone_level_db(&woofer_ir, 48_000.0, 200.0);
    let tweeter_low = tone_level_db(&tweeter_ir, 48_000.0, 200.0);
    let woofer_high = tone_level_db(&woofer_ir, 48_000.0, 8_000.0);
    let tweeter_high = tone_level_db(&tweeter_ir, 48_000.0, 8_000.0);
    println!(
        "listen passive: 200 Hz woofer {woofer_low:.1} / tweeter {tweeter_low:.1}; \
         8 kHz woofer {woofer_high:.1} / tweeter {tweeter_high:.1}"
    );
    assert!(
        woofer_low > tweeter_low + 10.0,
        "listen impulse: woofer should dominate at 200 Hz: {woofer_low} vs {tweeter_low}"
    );
    assert!(
        tweeter_high > woofer_high + 6.0,
        "listen impulse: tweeter should dominate at 8 kHz: {tweeter_high} vs {woofer_high}"
    );
}

/// Level of a pure tone in an impulse, via a one-bin DFT (dB re 1).
fn tone_level_db(impulse: &[f64], sample_rate_hz: f64, freq_hz: f64) -> f64 {
    let n = impulse.len() as f64;
    let omega = 2.0 * std::f64::consts::PI * freq_hz / sample_rate_hz;
    let mut re = 0.0;
    let mut im = 0.0;
    for (i, sample) in impulse.iter().enumerate() {
        let angle = omega * i as f64;
        re += sample * angle.cos();
        im -= sample * angle.sin();
    }
    let mag = (re * re + im * im).sqrt() / n;
    20.0 * mag.max(1e-30).log10()
}
