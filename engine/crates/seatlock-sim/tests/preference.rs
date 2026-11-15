//! CTA-2034 preference rating (README sections 2.6 and 11.1).

use seatlock_sim::preference::{
    low_frequency_extension_hz, narrow_band_deviation, preference_rating, smoothness,
};
use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::Value;

fn criton_asr() -> Value {
    let path = format!(
        "{}/../../../designs/imports/criton-3td-x/asr/cea2034.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Olive's formula with the Criton terms published on spinorama.org recovers the page score.
#[test]
fn olive_terms_from_the_criton_page_recover_its_preference_score() {
    let nbd_on = 0.594;
    let nbd_pir = 0.41;
    let sm_pir = 0.741;
    let lfx = (42.0f64).log10();
    let preference = 12.69 - 2.49 * nbd_on - 2.99 * nbd_pir - 4.31 * lfx + 2.32 * sm_pir;
    assert!(
        (preference - 4.68).abs() < 0.05,
        "preference {preference} against published 4.68"
    );
}

/// From ASR's Criton CTA-2034 curves: LFX near 42 Hz, NBD on-axis near 0.59, preference in band.
#[test]
fn preference_from_criton_asr_curves_is_near_the_published_score() {
    let file = criton_asr();
    let points = file["points"].as_array().unwrap();
    let mut freqs = Vec::new();
    let mut on = Vec::new();
    let mut lw = Vec::new();
    let mut er = Vec::new();
    let mut sp = Vec::new();
    for point in points {
        freqs.push(point["hz"].as_f64().unwrap());
        on.push(point["on_axis"].as_f64().unwrap());
        lw.push(point["listening_window"].as_f64().unwrap());
        er.push(point["early_reflections"].as_f64().unwrap());
        sp.push(point["sound_power"].as_f64().unwrap());
    }
    let lfx_hz = low_frequency_extension_hz(&freqs, &lw, &sp);
    let nbd_on = narrow_band_deviation(&freqs, &on);
    let score = preference_rating(&freqs, &on, &lw, &er, &sp);
    println!(
        "Criton ASR → preference {:.2} (pub 4.68), NBD_ON {:.3} (pub 0.594), LFX {:.1} Hz (pub 42), \
         SM_PIR {:.3}",
        score.preference, nbd_on, lfx_hz, score.sm_pir
    );
    assert!(
        (lfx_hz - 42.0).abs() < 3.0,
        "LFX {lfx_hz} Hz against published 42 Hz"
    );
    assert!(
        (nbd_on - 0.594).abs() < 0.05,
        "NBD on-axis {nbd_on} against published 0.594"
    );
    // Resampling details in spinorama's pipeline leave a few tenths on the composite score;
    // the formula itself is checked above.
    assert!(
        (score.preference - 4.68).abs() < 0.55,
        "preference {} against published 4.68",
        score.preference
    );
    assert!(
        score.sm_pir > 0.5 && score.sm_pir <= 1.0,
        "SM_PIR {}",
        score.sm_pir
    );
    let _ = smoothness(&freqs, &lw);
}

/// The everyday baffle run with the listening window forms CTA-2034 curves and a preference.
#[test]
fn a_baffle_run_reports_cta2034_preference() {
    let result = simulate(
        &example("sealed-test"),
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            listening_window: true,
            walls: false,
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let report = result
        .cta2034
        .as_ref()
        .expect("CTA-2034 report with the listening window");
    assert_eq!(report.on_axis_db.len(), result.frequencies_hz.len());
    assert_eq!(
        report.listening_window_db.len(),
        result.frequencies_hz.len()
    );
    assert_eq!(
        report.early_reflections_db.len(),
        result.frequencies_hz.len()
    );
    assert_eq!(report.sound_power_db.len(), result.frequencies_hz.len());
    assert_eq!(
        report.predicted_in_room_db.len(),
        result.frequencies_hz.len()
    );
    assert!(report.score.preference.is_finite());
    assert!(report.score.lfx_hz > 0.0);
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|line| line.starts_with("CTA-2034 preference")));
    println!(
        "sealed-test preference {:.2}, LFX {:.0} Hz, NBD_ON {:.2}",
        report.score.preference, report.score.lfx_hz, report.score.nbd_on
    );
}
