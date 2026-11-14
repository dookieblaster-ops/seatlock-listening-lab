//! The LCM Nearfield Monitor by zx82net, imported from its designer's own model files
//! (designs/imports/lcm, CC BY-NC): an imported cabinet with holes its drivers fill, two
//! sealed air spaces, and a 55 cm port moulded into it (README 6.2, `mesh`).

use seatlock_core::issue::Severity;
use seatlock_core::DesignFile;
use seatlock_sim::{builtin, simulate, SimulateRequest};

fn load(name: &str) -> DesignFile {
    let path = format!(
        "{}/../../../designs/imports/lcm/{name}.seatlock.json",
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

#[test]
fn the_lcm_imports_with_the_air_spaces_and_port_its_designer_published() {
    // The designer's build notes: a 1.85 L bass enclosure tuned by a folded 55 cm port, and a
    // separate 0.4 L space behind the tweeter.
    let registry = builtin();
    for name in ["lcm-evo", "lcm-2021"] {
        let design = load(name);
        let outer = registry
            .create_outer(
                &design.speaker.enclosure.outer.kind,
                &design.speaker.enclosure.outer.fields,
            )
            .unwrap();
        let air = outer.air_regions().unwrap();
        let openings = outer.moulded_openings();
        assert_eq!(openings.len(), 1);
        let port = &openings[0];
        let bass = outer.region_behind("woofer-hole").unwrap();
        let tweeter = outer.region_behind("tweeter-hole").unwrap();
        assert_ne!(bass, tweeter);
        assert_eq!(port.region, bass);
        let net = air.volumes_m3[bass] - port.volume_m3();
        eprintln!(
            "{name}: bass {:.3} L net, tweeter {:.3} L, port {:.1} mm long, {:.2} cm^2 average",
            net * 1e3,
            air.volumes_m3[tweeter] * 1e3,
            port.length_m() * 1e3,
            port.length_m() / port.length_over_area() * 1e4
        );
        assert!((net - 1.85e-3).abs() < 0.03 * 1.85e-3, "{name}: bass {net}");
        assert!(
            (air.volumes_m3[tweeter] - 0.4e-3).abs() < 0.05 * 0.4e-3,
            "{name}: tweeter {}",
            air.volumes_m3[tweeter]
        );
        assert!(
            (port.length_m() - 0.55).abs() < 0.03 * 0.55,
            "{name}: port {}",
            port.length_m()
        );
        let r = simulate(&design, &everyday(), &registry);
        assert!(
            r.issues.iter().all(|i| i.severity != Severity::Error),
            "{name}: {:?}",
            r.issues
        );
        assert!(r
            .fidelity
            .ignored
            .iter()
            .any(|l| l.contains("moulded into the imported model")));
    }
}

/// Bass extension as spinorama.org reports it: relative to the mean level from 300 Hz to
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

#[test]
fn the_lcms_bass_reaches_about_as_low_as_erin_measured() {
    // Erin's Audio Corner measured the speaker on a Klippel Near-Field Scanner (review of
    // 2022-06-22); spinorama.org gives its response 6 dB down at 33.7 Hz from its level
    // between 300 Hz and 5 kHz. This is a comparison with a measurement, not a closed form:
    // it holds the importer, the moulded port, and the drivers' data together.
    let design = load("lcm-evo");
    let r = simulate(&design, &everyday(), &builtin());
    let f6 = extension(&r.frequencies_hz, &r.summed_spl_db, 6.0);
    eprintln!("lcm-evo: 6 dB down at {f6:.1} Hz (measured 33.7 Hz)");
    assert!((f6 - 33.7).abs() < 0.08 * 33.7, "{f6}");
}

/// Erin's CTA-2034 curves (designs/imports/lcm/erin/cea2034.json): relative dB vs Hz.
fn erin_curve(name: &str) -> (Vec<f64>, Vec<f64>) {
    let path = format!(
        "{}/../../../designs/imports/lcm/erin/cea2034.json",
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
fn the_lcm_matches_erins_shape_within_known_limits() {
    // Owner (2026-11-14): yes to keeping Erin's measured curves (spinorama / CC BY-NC-SA).
    // Align both curves on their 300 Hz to 5 kHz means and compare shapes. Erin's on-axis
    // bass (40 to 270 Hz) sits about level with its midband; the lab's is about 3 dB low
    // there. Directivity (window vs axis, early reflections, sound power) is not compared
    // yet: the lab's everyday request has the listening window but not the full CTA-2034 set.
    let (erin_f, erin_on) = erin_curve("on_axis");
    let erin_mid = mean_in(&erin_f, &erin_on, 300.0, 5000.0);
    let erin_bass = mean_in(&erin_f, &erin_on, 40.0, 270.0) - erin_mid;
    let erin_f3 = extension(&erin_f, &erin_on, 3.0);
    let erin_f6 = extension(&erin_f, &erin_on, 6.0);

    let design = load("lcm-evo");
    let r = simulate(
        &design,
        &SimulateRequest {
            listening_window: true,
            ..everyday()
        },
        &builtin(),
    );
    let lab_mid = mean_in(&r.frequencies_hz, &r.summed_spl_db, 300.0, 5000.0);
    let lab_bass = mean_in(&r.frequencies_hz, &r.summed_spl_db, 40.0, 270.0) - lab_mid;
    let lab_aligned: Vec<f64> = r.summed_spl_db.iter().map(|s| s - lab_mid).collect();

    // Per-band RMS of lab against Erin after midband alignment (lab frequencies).
    let mut rms = [0.0; 3];
    let mut count = [0.0; 3];
    for (f, lab) in r.frequencies_hz.iter().zip(&lab_aligned) {
        let erin = interpolate(&erin_f, &erin_on, *f) - erin_mid;
        let err = (lab - erin).powi(2);
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
    let tweeter_band = mean_in(&r.frequencies_hz, tweeter, 400.0, 2000.0);

    eprintln!(
        "Erin on-axis: bass-mid {erin_bass:+.2} dB, f3={erin_f3:.1} Hz, f6={erin_f6:.1} Hz (reported 38.1 / 33.7)"
    );
    eprintln!("Lab on-axis:  bass-mid {lab_bass:+.2} dB (Erin is near 0; lab is low)");
    eprintln!(
        "RMS after midband align: <300 Hz {:.2} dB, 0.3-2 kHz {:.2} dB, 2-10 kHz {:.2} dB",
        rms[0], rms[1], rms[2]
    );
    eprintln!(
        "Lab radiators (absolute): woofer 80-180 Hz {woofer_band:.1} dB, BMR 400-2000 Hz {tweeter_band:.1} dB, gap {:+.1} dB",
        tweeter_band - woofer_band
    );

    // Erin's published -6 dB point (spinorama summary 33.7 Hz) matches the stored curve.
    // Their -3 dB summary (38.1 Hz) is a little higher than this file's on-axis crossing
    // (~36.7 Hz); both are recorded. Bass sits level with the midband on Erin's curve.
    assert!((erin_f6 - 33.7).abs() < 0.5, "erin f6 {erin_f6}");
    assert!((erin_f3 - 36.7).abs() < 1.0, "erin f3 from file {erin_f3}");
    assert!(erin_bass.abs() < 1.0, "erin bass-mid {erin_bass}");
    // The lab's known shape fault: bass a few dB below midband. Placed 2026-11-14: the BMR's
    // own band sits about 3 dB above the woofer's, so the summed midrange rides high. Midband
    // shape against Erin is already close; the bass band is not.
    assert!(
        lab_bass < -1.5 && lab_bass > -4.0,
        "lab bass-mid {lab_bass}"
    );
    assert!(
        (tweeter_band - woofer_band - 2.9).abs() < 1.0,
        "BMR–woofer gap {}",
        tweeter_band - woofer_band
    );
    assert!(rms[1] < 3.0, "mid RMS {}", rms[1]);
    assert!(
        rms[0] > 3.0,
        "bass RMS should show the known fault: {}",
        rms[0]
    );
}

#[test]
#[ignore = "about 8 minutes: the 3D grid at 5 mm, which the 10 mm port slot needs"]
fn the_lcms_full_3d_model_agrees_with_the_detailed_model() {
    let registry = builtin();
    let design = load("lcm-evo");
    let detailed = simulate(&design, &everyday(), &registry);
    let r = simulate(
        &design,
        &SimulateRequest {
            enclosure: "fdtd".into(),
            fdtd_dx_m: Some(0.005),
            ..everyday()
        },
        &registry,
    );
    assert!(r.issues.iter().all(|i| i.severity != Severity::Error));
    let worst = |lo: f64, hi: f64| {
        r.frequencies_hz
            .iter()
            .zip(r.summed_spl_db.iter().zip(&detailed.summed_spl_db))
            .filter(|(f, _)| **f >= lo && **f <= hi)
            .map(|(_, (a, b))| (a - b).abs())
            .fold(0.0, f64::max)
    };
    let (wide, upper) = (worst(30.0, 400.0), worst(44.0, 400.0));
    eprintln!("3D against detailed: {wide:.2} dB from 30 Hz, {upper:.2} dB from 44 Hz");
    assert!(wide < 2.5 && upper < 1.2);
}
