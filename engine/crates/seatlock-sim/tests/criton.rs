//! CSS Criton 3TD-X kit copy vs ASR CTA-2034 (designs/imports/criton-3td-x).

use seatlock_core::issue::Severity;
use seatlock_core::DesignFile;
use seatlock_sim::{builtin, simulate, SimulateRequest};

fn load() -> DesignFile {
    let path = format!(
        "{}/../../../designs/imports/criton-3td-x/criton-3td-x.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn asr_curve(name: &str) -> (Vec<f64>, Vec<f64>) {
    let path = format!(
        "{}/../../../designs/imports/criton-3td-x/asr/cea2034.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut freqs = Vec::new();
    let mut levels = Vec::new();
    for point in raw["points"].as_array().unwrap() {
        freqs.push(point["hz"].as_f64().unwrap());
        levels.push(point[name].as_f64().unwrap());
    }
    (freqs, levels)
}

fn mean_in(freqs: &[f64], levels: &[f64], lo: f64, hi: f64) -> f64 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for (f, s) in freqs.iter().zip(levels) {
        if *f >= lo && *f <= hi {
            sum += s;
            n += 1.0;
        }
    }
    sum / n
}

fn interpolate(freqs: &[f64], levels: &[f64], at: f64) -> f64 {
    if at <= freqs[0] {
        return levels[0];
    }
    if at >= *freqs.last().unwrap() {
        return *levels.last().unwrap();
    }
    let i = freqs.partition_point(|f| *f < at).saturating_sub(1);
    let (f0, f1) = (freqs[i], freqs[i + 1]);
    let (s0, s1) = (levels[i], levels[i + 1]);
    let t = ((at / f0).ln()) / ((f1 / f0).ln());
    s0 + t * (s1 - s0)
}

#[test]
fn the_criton_simulates_with_its_passive_network() {
    let design = load();
    assert!(design.dsp.passive.is_some());
    assert_eq!(design.speaker.radiators.len(), 4);
    let r = simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            losses: true,
            frequencies_hz: Some(vec![80.0, 1000.0, 8000.0]),
            ..Default::default()
        },
        &builtin(),
    );
    assert!(
        r.issues.iter().all(|i| i.severity != Severity::Error),
        "{:?}",
        r.issues
    );
    assert_eq!(r.radiators.len(), 4);
    let low_woofers = (r.radiators[0].spl_db[0] + r.radiators[1].spl_db[0]) / 2.0;
    let low_tweet = r.radiators[3].spl_db[0];
    let high_tweet = r.radiators[3].spl_db[2];
    let high_woofers = (r.radiators[0].spl_db[2] + r.radiators[1].spl_db[2]) / 2.0;
    eprintln!(
        "criton: 80 Hz woofers {low_woofers:.1} tweeter {low_tweet:.1}; \
         8 kHz woofers {high_woofers:.1} tweeter {high_tweet:.1}"
    );
    assert!(
        low_woofers > low_tweet + 10.0,
        "{low_woofers} vs {low_tweet}"
    );
    assert!(
        high_tweet > high_woofers + 6.0,
        "{high_tweet} vs {high_woofers}"
    );
    assert!((r.radiators[0].spl_db[0] - r.radiators[1].spl_db[0]).abs() < 1.0);
}

#[test]
fn the_criton_matches_asr_shape_within_known_limits() {
    let (asr_f, asr_on) = asr_curve("on_axis");
    let asr_mid = mean_in(&asr_f, &asr_on, 300.0, 5000.0);
    let asr_bass = mean_in(&asr_f, &asr_on, 40.0, 270.0) - asr_mid;
    let design = load();
    let r = simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            losses: true,
            listening_window: true,
            ..Default::default()
        },
        &builtin(),
    );
    assert!(
        r.issues.iter().all(|i| i.severity != Severity::Error),
        "{:?}",
        r.issues
    );
    let lab_mid = mean_in(&r.frequencies_hz, &r.summed_spl_db, 300.0, 5000.0);
    let lab_bass = mean_in(&r.frequencies_hz, &r.summed_spl_db, 40.0, 270.0) - lab_mid;
    let lab_aligned: Vec<f64> = r.summed_spl_db.iter().map(|s| s - lab_mid).collect();
    let mut rms = [0.0; 3];
    let mut count = [0.0; 3];
    for (f, lab) in r.frequencies_hz.iter().zip(&lab_aligned) {
        let asr = interpolate(&asr_f, &asr_on, *f) - asr_mid;
        let err = (lab - asr).powi(2);
        let band = if *f < 300.0 {
            0
        } else if *f <= 2000.0 {
            1
        } else if *f <= 10000.0 {
            2
        } else {
            continue;
        };
        rms[band] += err;
        count[band] += 1.0;
    }
    for i in 0..3 {
        rms[i] = (rms[i] / count[i]).sqrt();
    }
    eprintln!("ASR bass-mid {asr_bass:+.2} dB; lab bass-mid {lab_bass:+.2} dB");
    eprintln!(
        "RMS after midband align: <300 Hz {:.2} dB, 0.3-2 kHz {:.2} dB, 2-10 kHz {:.2} dB",
        rms[0], rms[1], rms[2]
    );
    // Placed 2026-11-15: after midband align, mid and treble are within about 2 dB RMS of
    // ASR. Bass sits high (lab about +5 dB vs ASR about −0.5 dB relative to the 300 Hz–5 kHz
    // mean). Limits: partition height estimated, port is the drawing's straight 3×8 in
    // substitute (not the kit's flared tube), no maker response curves, no cabinet fill.
    assert!(asr_bass > -2.0 && asr_bass < 1.0, "asr bass-mid {asr_bass}");
    assert!(lab_bass > 3.0 && lab_bass < 8.0, "lab bass-mid {lab_bass}");
    assert!(rms[0] > 4.0 && rms[0] < 8.0, "bass RMS {}", rms[0]);
    assert!(rms[1] < 3.0, "mid RMS {}", rms[1]);
    assert!(rms[2] < 3.0, "treble RMS {}", rms[2]);
    let _ = lab_mid;
}
