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
