use seatlock_core::constants::{C0, RHO0};
use seatlock_fdtd::{aperture_cells, magnitude_peaks, run_cavity, run_grid, Cavity};
use std::f64::consts::PI;

fn reference_cavity() -> Cavity {
    Cavity {
        size_m: [0.30, 0.20, 0.15],
        dx: 0.01,
        patch_origin: [0, 0],
        patch_cells: [4, 4],
    }
}

#[test]
fn chk_fdtd_1_box_resonances() {
    let curve = run_cavity(&reference_cavity());
    let peaks = magnitude_peaks(&curve);
    eprintln!("steps {} peaks {:?}", curve.steps, peaks);
    let expected = [572.0, 858.0, 1031.2, 1144.0, 1279.0, 1430.0];
    for target in expected {
        let found = peaks
            .iter()
            .any(|peak| (peak - target).abs() <= target * 0.005);
        assert!(found, "missing {target} among {peaks:?}");
    }
}

#[test]
fn chk_fdtd_2_low_frequency_compliance() {
    let curve = run_cavity(&reference_cavity());
    let volume = 0.30 * 0.20 * 0.15;
    let compliance = volume / (RHO0 * C0 * C0);
    let freq = 10.0;
    let omega = 2.0 * PI * freq;
    let expected_mag = 1.0 / (omega * compliance);
    let index = curve
        .frequencies_hz
        .iter()
        .position(|value| *value >= freq)
        .unwrap()
        .max(1);
    let f0 = curve.frequencies_hz[index - 1];
    let f1 = curve.frequencies_hz[index];
    let mix = (freq - f0) / (f1 - f0);
    let re = curve.real[index - 1] + mix * (curve.real[index] - curve.real[index - 1]);
    let im = curve.imag[index - 1] + mix * (curve.imag[index] - curve.imag[index - 1]);
    let mag = (re * re + im * im).sqrt();
    let phase = im.atan2(re).to_degrees();
    eprintln!("interp {freq} Hz mag {mag} vs {expected_mag} phase {phase}");
    assert!(
        (mag - expected_mag).abs() <= 0.01 * expected_mag,
        "{mag} vs {expected_mag}"
    );
    assert!((phase + 90.0).abs() <= 1.0, "phase {phase}");
    assert!(curve.warning.is_none(), "{:?}", curve.warning);
}

#[test]
fn chk_fdtd_3_helmholtz_neck() {
    let curve = seatlock_fdtd::helmholtz_neck(3.0);
    eprintln!("FDTD3 steps {} ms {}", curve.steps, curve.elapsed_ms);
    let mut best_f = 0.0;
    let mut best = 0.0;
    for index in 0..curve.frequencies_hz.len() {
        let freq = curve.frequencies_hz[index];
        if !(40.0..=120.0).contains(&freq) {
            continue;
        }
        let mag = (curve.z_real[index].powi(2) + curve.z_imag[index].powi(2)).sqrt();
        if mag > best {
            best = mag;
            best_f = freq;
        }
    }
    eprintln!("FDTD3 peak {best_f} Hz");
    assert!((best_f - 65.7).abs() / 65.7 <= 0.02, "peak {best_f}");
    let sample = |freq: f64| -> (f64, f64) {
        let index = curve
            .frequencies_hz
            .iter()
            .position(|value| *value >= freq)
            .unwrap()
            .max(1);
        let f0 = curve.frequencies_hz[index - 1];
        let f1 = curve.frequencies_hz[index];
        let mix = (freq - f0) / (f1 - f0);
        let re = curve.h_real[index - 1] + mix * (curve.h_real[index] - curve.h_real[index - 1]);
        let im = curve.h_imag[index - 1] + mix * (curve.h_imag[index] - curve.h_imag[index - 1]);
        (re, im)
    };
    let (re10, im10) = sample(10.0);
    let (re20, im20) = sample(20.0);
    eprintln!("H 10 Hz {re10} {im10}; 20 Hz {re20} {im20}");
    assert!((re10 - 1.024).abs() <= 0.01 * 1.024, "H10 {re10}");
    assert!(im10.abs() < 0.05, "im10 {im10}");
    assert!((re20 - 1.103).abs() <= 0.01 * 1.103, "H20 {re20}");
    assert!(im20.abs() < 0.05, "im20 {im20}");
}

#[test]
#[ignore = "larger exterior block; run with --release -- --ignored to record the size check"]
fn exterior_block_doubling_changes_the_peak_by_under_one_percent() {
    let small = seatlock_fdtd::helmholtz_neck(3.0);
    let large = seatlock_fdtd::helmholtz_neck(6.0);
    let peak = |curve: &seatlock_fdtd::OpeningCurve| {
        let mut best_f = 0.0;
        let mut best = 0.0;
        for index in 0..curve.frequencies_hz.len() {
            let freq = curve.frequencies_hz[index];
            if !(40.0..=120.0).contains(&freq) {
                continue;
            }
            let mag = (curve.z_real[index].powi(2) + curve.z_imag[index].powi(2)).sqrt();
            if mag > best {
                best = mag;
                best_f = freq;
            }
        }
        (best_f, best)
    };
    let (f_small, z_small) = peak(&small);
    let (f_large, z_large) = peak(&large);
    let freq_change = (f_large - f_small).abs() / f_small;
    let level_change = (z_large - z_small).abs() / z_small;
    eprintln!(
        "BLOCK small {f_small} Hz {z_small}; large {f_large} Hz {z_large}; freq {freq_change} level {level_change}"
    );
    assert!(freq_change < 0.01, "frequency moved by {freq_change}");
}

#[test]
fn sealed_test_box_at_10mm() {
    let dx: f64 = 0.01;
    let depth = 0.180 - 2.0 * 0.008;
    let width = 0.160 - 2.0 * 0.008;
    let height = 0.250 - 2.0 * 0.008;
    let nx = (depth / dx).round() as usize;
    let ny = (width / dx).round() as usize;
    let nz = (height / dx).round() as usize;
    let source = aperture_cells(ny, nz, dx, [0.080 - 0.008, 0.150 - 0.008], 0.059);
    let true_volume = depth * width * height;
    let curve = run_grid(nx, ny, nz, dx, &source, true_volume);
    eprintln!("SEALED_TEST_10MM_MS {}", curve.elapsed_ms);
    assert!(curve.steps > 100);
    let warning = curve.warning.expect("sealed-test grid should warn");
    assert!(warning.contains("L"), "{warning}");
    assert!(curve.ignored.iter().any(|item| item.contains("differs by")));
}
