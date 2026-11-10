use seatlock_acoustics::{
    derive_mechanical, reference_half_space_spl_db, sealed_alignment, sealed_point, Published,
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
