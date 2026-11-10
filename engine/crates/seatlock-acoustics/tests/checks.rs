use seatlock_acoustics::{
    derive_mechanical, reference_half_space_spl_db, sealed_alignment, sealed_point, vented_point,
    PortSpec, Published,
};
use seatlock_core::grid::log_grid;

fn woofer_a() -> Published {
    Published {
        fs_hz: 50.0,
        qes: 0.45,
        qms: 4.0,
        vas_m3: 0.010,
        re_ohm: 6.0,
        le_h: 0.0005,
        sd_m2: 0.0085,
        xmax_m: 0.004,
        pe_w: 30.0,
    }
}

fn woofer_b() -> Published {
    Published {
        fs_hz: 40.0,
        qes: 0.4144,
        qms: 5.0,
        vas_m3: 0.020,
        re_ohm: 6.0,
        le_h: 0.0,
        sd_m2: 0.013,
        xmax_m: 0.006,
        pe_w: 50.0,
    }
}

fn close_rel(got: f64, expected: f64, relative: f64) {
    let tolerance = expected.abs() * relative;
    assert!(
        (got - expected).abs() <= tolerance,
        "{got} vs {expected} (relative {relative})"
    );
}

#[test]
fn chk_drv_1_mechanical_parameters() {
    let a = derive_mechanical(&woofer_a());
    close_rel(a.cms, 9.7598e-4, 0.001);
    close_rel(a.mms, 0.010381, 0.001);
    close_rel(a.rms, 0.8154, 0.001);
    close_rel(a.bl, 6.594, 0.001);
    close_rel(a.qts, 0.4045, 0.001);

    let b = derive_mechanical(&woofer_b());
    close_rel(b.cms, 8.3449e-4, 0.001);
    close_rel(b.mms, 0.018971, 0.001);
    close_rel(b.rms, 0.9536, 0.001);
    close_rel(b.bl, 8.309, 0.001);
    close_rel(b.qts, 0.3827, 0.001);
}

#[test]
fn chk_drv_2_reference_efficiency_at_1khz() {
    let mut published = woofer_a();
    published.le_h = 0.0;
    let (spl, _, _) = sealed_point(&published, 0.005, 2.83, 1_000.0, "half", 1.0, 0.16);
    let expected = reference_half_space_spl_db(&published, 2.83, 1.0);
    assert!(
        (expected - 87.75).abs() < 0.02,
        "formula {expected} is not the published 87.75"
    );
    assert!(
        (spl - expected).abs() <= 0.02,
        "spl {spl} vs formula {expected}"
    );
}

#[test]
fn chk_box_1_sealed_highpass() {
    let mut published = woofer_a();
    published.le_h = 0.0;
    let (fc, qtc) = sealed_alignment(&published, 0.005);
    assert!((fc - 86.60).abs() < 0.01, "fc {fc}");
    assert!((qtc - 0.7006).abs() < 0.0001, "qtc {qtc}");
    let level = reference_half_space_spl_db(&published, 2.83, 1.0);
    for freq in log_grid(5.0, 20_000.0, 24.0) {
        let (spl, _, _) = sealed_point(&published, 0.005, 2.83, freq, "half", 1.0, 0.16);
        let s = num_complex_s(freq, fc);
        let highpass = (s * s) / (s * s + s / qtc + 1.0);
        let expected = level + 20.0 * highpass.norm().log10();
        assert!(
            (spl - expected).abs() <= 0.001,
            "{freq} Hz: {spl} vs {expected}"
        );
    }
}

fn chk_box_3_port() -> PortSpec {
    let radius = 0.0175;
    PortSpec {
        area_m2: std::f64::consts::PI * radius * radius,
        length_eff_m: 0.12686,
    }
}

#[test]
fn chk_box_3_vented_butterworth() {
    let published = woofer_b();
    let volume = 0.020 / 2.0_f64.sqrt();
    let level = reference_half_space_spl_db(&published, 2.83, 1.0);
    assert!((level - 88.21).abs() < 0.02, "level {level}");
    let (low_spl, _, _, _, h) = vented_point(
        &published,
        volume,
        chk_box_3_port(),
        2.83,
        1.0,
        "half",
        1.0,
        0.2,
    );
    assert!((h - 1.0).abs() < 0.01, "H_port at 1 Hz is {h}");
    let _ = low_spl;
    for freq in log_grid(8.0, 800.0, 24.0) {
        let (spl, _, _, _, _) = vented_point(
            &published,
            volume,
            chk_box_3_port(),
            2.83,
            freq,
            "half",
            1.0,
            0.2,
        );
        let ratio = 40.0 / freq;
        let magnitude = 1.0 / (1.0 + ratio.powi(8)).sqrt();
        let expected = level + 20.0 * magnitude.max(1e-30).log10();
        assert!(
            (spl - expected).abs() <= 0.001,
            "{freq} Hz: {spl} vs {expected}"
        );
    }
}

#[test]
fn chk_box_4_vented_samples() {
    let published = woofer_b();
    let volume = 0.020 / 2.0_f64.sqrt();
    let cases = [
        (20.0, 64.11, 3.462e-3, 7.84),
        (40.0, 85.20, 0.0005e-3, 11.11),
        (80.0, 88.19, 0.866e-3, 1.96),
    ];
    for (freq, spl_db, excursion_m, speed) in cases {
        let (spl, impedance, excursion, port, _) = vented_point(
            &published,
            volume,
            chk_box_3_port(),
            2.83,
            freq,
            "half",
            1.0,
            0.2,
        );
        assert!((spl - spl_db).abs() <= 0.02, "{freq} spl {spl}");
        if freq == 40.0 {
            assert!((impedance - 6.0).abs() <= 0.03, "z {impedance}");
            assert!(excursion < 0.001e-3, "x {excursion}");
        } else {
            let travel_tol = excursion_m * 0.005;
            assert!(
                (excursion - excursion_m).abs() <= travel_tol.max(1e-6),
                "{freq} x {excursion} vs {excursion_m}"
            );
        }
        let speed_tol = speed * 0.005;
        assert!(
            (port - speed).abs() <= speed_tol,
            "{freq} port {port} vs {speed}"
        );
    }
    let peak_near = |low: f64, high: f64| {
        let mut best_f = low;
        let mut best_z = 0.0;
        let mut freq = low;
        while freq <= high {
            let (_, impedance, _, _, _) = vented_point(
                &published,
                volume,
                chk_box_3_port(),
                2.83,
                freq,
                "half",
                1.0,
                0.2,
            );
            if impedance > best_z {
                best_z = impedance;
                best_f = freq;
            }
            freq += 0.02;
        }
        best_f
    };
    let low_peak = peak_near(18.0, 30.0);
    let high_peak = peak_near(60.0, 85.0);
    assert!((low_peak - 22.8).abs() <= 0.2, "low peak {low_peak}");
    assert!((high_peak - 70.4).abs() <= 0.2, "high peak {high_peak}");
}

#[test]
fn sourced_drivers_match_printed_mechanics_or_would_warn() {
    for id in ["dayton-tcp115-4", "dayton-ps95-8", "dayton-rs100-8"] {
        let record = seatlock_core::catalog::driver(id).expect(id);
        assert_eq!(record["status"], "sourced");
        assert_eq!(record["source"]["read_on"], "2026-11-10");
        assert_eq!(record["price"]["read_on"], "2026-11-10");
        let published = seatlock_acoustics::published_from_catalog(&record).unwrap();
        let mech = derive_mechanical(&published);
        let printed = &record["published"];
        close_rel(mech.mms, printed["mms_kg"].as_f64().unwrap(), 0.1);
        close_rel(mech.cms, printed["cms_m_per_n"].as_f64().unwrap(), 0.1);
        close_rel(mech.bl, printed["bl_tm"].as_f64().unwrap(), 0.1);
    }
}

fn num_complex_s(freq: f64, fc: f64) -> num_complex::Complex64 {
    num_complex::Complex64::new(0.0, freq / fc)
}

#[test]
fn chk_box_2_published_driver_in_5l() {
    let published = woofer_a();
    let cases = [
        (40.0, 74.02, 7.87, 1.381e-3),
        (86.60, 84.66, 59.33, 1.003e-3),
        (1_000.0, 87.12, 6.50, 0.0100e-3),
    ];
    for (freq, spl_db, ohms, excursion_m) in cases {
        let (spl, impedance, excursion) =
            sealed_point(&published, 0.005, 2.83, freq, "half", 1.0, 0.16);
        assert!(
            (spl - spl_db).abs() <= 0.02,
            "{freq} Hz spl {spl} vs {spl_db}"
        );
        close_rel(impedance, ohms, 0.005);
        close_rel(excursion, excursion_m, 0.005);
    }
}
