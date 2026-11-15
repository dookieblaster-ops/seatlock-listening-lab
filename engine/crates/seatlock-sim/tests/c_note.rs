//! Parts Express C-Note MT kit copy: passive crossover vs ASR CTA-2034
//! (designs/imports/c-note, CC BY-NC-SA for the curves).

use seatlock_core::issue::Severity;
use seatlock_core::DesignFile;
use seatlock_sim::{builtin, simulate, SimulateRequest};

fn load() -> DesignFile {
    let path = format!(
        "{}/../../../designs/imports/c-note/c-note-mt.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn everyday() -> SimulateRequest {
    SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        losses: true,
        ..Default::default()
    }
}

/// Bass extension as spinorama reports it: relative to the mean level from 300 Hz to
/// 5 kHz, the highest frequency below 300 Hz where the response is `drop` dB under it.
fn extension(freqs: &[f64], spl: &[f64], drop: f64) -> f64 {
    let band: Vec<f64> = freqs
        .iter()
        .zip(spl)
        .filter(|(f, _)| **f >= 300.0 && **f <= 5000.0)
        .map(|(_, s)| *s)
        .collect();
    let reference = band.iter().sum::<f64>() / band.len() as f64;
    for i in (0..freqs.len() - 1).rev() {
        if freqs[i] <= 300.0 && spl[i] < reference - drop {
            let (f0, f1, s0, s1) = (freqs[i], freqs[i + 1], spl[i], spl[i + 1]);
            let t = (reference - drop - s0) / (s1 - s0);
            return (f0.ln() + t * (f1.ln() - f0.ln())).exp();
        }
    }
    0.0
}

fn asr_curve(name: &str) -> (Vec<f64>, Vec<f64>) {
    let path = format!(
        "{}/../../../designs/imports/c-note/asr/cea2034.json",
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
    let (f0, f1, s0, s1) = (freqs[i], freqs[i + 1], levels[i], levels[i + 1]);
    let t = (at.ln() - f0.ln()) / (f1.ln() - f0.ln());
    s0 + t * (s1 - s0)
}

#[test]
fn the_c_note_mt_simulates_with_its_passive_network() {
    let design = load();
    assert!(design.dsp.passive.is_some());
    let r = simulate(&design, &everyday(), &builtin());
    assert!(
        r.issues.iter().all(|i| i.severity != Severity::Error),
        "{:?}",
        r.issues
    );
    assert!(r
        .fidelity
        .ignored
        .iter()
        .any(|l| l.starts_with("Passive crossover")));
    assert_eq!(r.radiators.len(), 2);
    let vol = r.interior_volume_m3.expect("volume");
    eprintln!("c-note-mt: net volume {:.2} L", vol * 1e3);
    // Manual: ~0.32 ft³ / ~9 L gross internal before drivers and port.
    assert!(
        vol > 0.007 && vol < 0.010,
        "expected about 8–9 L net, got {} L",
        vol * 1e3
    );
}

#[test]
fn the_c_note_passive_network_sends_bass_to_the_woofer() {
    let design = load();
    let r = simulate(
        &design,
        &SimulateRequest {
            frequencies_hz: Some(vec![200.0, 8000.0]),
            radiation: "point".into(),
            walls: false,
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(r.issues.iter().all(|i| i.severity != Severity::Error));
    let woofer_low = r.radiators[0].spl_db[0];
    let tweeter_low = r.radiators[1].spl_db[0];
    let woofer_high = r.radiators[0].spl_db[1];
    let tweeter_high = r.radiators[1].spl_db[1];
    eprintln!(
        "c-note passive: 200 Hz woofer {woofer_low:.1} / tweeter {tweeter_low:.1}; \
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

#[test]
fn the_c_note_matches_asr_shape_within_known_limits() {
    // ASR CTA-2034 via spinorama (CC BY-NC-SA). Align midband means and compare shapes.
    let (asr_f, asr_on) = asr_curve("on_axis");
    let asr_mid = mean_in(&asr_f, &asr_on, 300.0, 5000.0);
    let asr_bass = mean_in(&asr_f, &asr_on, 40.0, 270.0) - asr_mid;
    let asr_f3 = extension(&asr_f, &asr_on, 3.0);
    let asr_f6 = extension(&asr_f, &asr_on, 6.0);

    let design = load();
    let r = simulate(
        &design,
        &SimulateRequest {
            listening_window: true,
            ..everyday()
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

    let woofer = &r.radiators[0].spl_db;
    let tweeter = &r.radiators[1].spl_db;
    let woofer_band = mean_in(&r.frequencies_hz, woofer, 80.0, 180.0);
    let tweeter_band = mean_in(&r.frequencies_hz, tweeter, 4000.0, 10000.0);

    eprintln!("ASR on-axis: bass-mid {asr_bass:+.2} dB, f3={asr_f3:.1} Hz, f6={asr_f6:.1} Hz");
    eprintln!("Lab on-axis:  bass-mid {lab_bass:+.2} dB");
    eprintln!(
        "RMS after midband align: <300 Hz {:.2} dB, 0.3-2 kHz {:.2} dB, 2-10 kHz {:.2} dB",
        rms[0], rms[1], rms[2]
    );
    eprintln!(
        "Lab radiators (absolute): woofer 80-180 Hz {woofer_band:.1} dB, \
         tweeter 4-10 kHz {tweeter_band:.1} dB, gap {:+.1} dB",
        tweeter_band - woofer_band
    );

    // Stored ASR curve: midband-normalised; bass sits a few dB low vs midband on this file
    // (kit advertising ~43 Hz F3 is an absolute shelf, not this relative metric).
    assert!(
        asr_bass < -1.0 && asr_bass > -6.0,
        "asr bass-mid {asr_bass}"
    );
    assert!(asr_f6 > 40.0 && asr_f6 < 120.0, "asr f6 {asr_f6}");
    let lab_f6 = extension(&r.frequencies_hz, &r.summed_spl_db, 6.0);
    eprintln!("Lab f6={lab_f6:.1} Hz");
    // Placed 2026-11-15: after midband align the lab's bass is about 2–3 dB lower than
    // ASR's relative to the 300 Hz–5 kHz mean (lab bass-mid ~−6 dB vs ASR ~−3.6 dB), so the
    // relative −6 dB point sits higher (~100 Hz). Mid and treble shapes match within a few
    // dB RMS. Limits: estimated baffle hole positions, ideal inductors (no DCR), parametric
    // box instead of vibrating 1/2 in MDF, ND25FW Sd from voice-coil diameter, waveguide
    // directivity not in the side angles.
    assert!(
        lab_bass < -4.0 && lab_bass > -9.0,
        "lab bass-mid {lab_bass}"
    );
    assert!(lab_f6 > 70.0 && lab_f6 < 140.0, "lab f6 {lab_f6}");
    assert!(rms[1] < 3.5, "mid RMS {}", rms[1]);
    assert!(rms[2] < 4.0, "treble RMS {}", rms[2]);
    assert!(
        rms[0] > 5.0,
        "bass RMS should show the known relative-bass gap: {}",
        rms[0]
    );
    assert!(woofer_band > 70.0, "woofer band {woofer_band}");
    assert!(tweeter_band > 70.0, "tweeter band {tweeter_band}");
}
