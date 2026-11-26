//! C6 dynamic bass: look-ahead limiter from cone travel X/V.

use seatlock_dsp::dynamic_bass_block;
use std::f64::consts::PI;

fn test_woofer_mech() -> (f64, f64, f64, f64, f64, f64, f64) {
    let re = 6.0;
    let le = 0.0005;
    let fs = 50.0;
    let qms = 4.0;
    let qes = 0.45;
    let vas = 0.01;
    let sd = 0.0085;
    let xmax = 0.004;
    let rho = 1.204;
    let c0 = 343.2;
    let cms = vas / (rho * c0 * c0 * sd * sd);
    let omega_s = 2.0 * PI * fs;
    let mms = 1.0 / (omega_s * omega_s * cms);
    let rms = omega_s * mms / qms;
    let bl = (omega_s * mms * re / qes).sqrt();
    (xmax, re, le, mms, cms, rms, bl)
}

#[test]
fn small_signal_is_a_pure_look_ahead_delay() {
    let rate = 48_000.0;
    let look_ms = 5.0_f64;
    let look = (look_ms * 1e-3 * rate).round() as usize;
    let (xmax, re, le, mms, cms, rms, bl) = test_woofer_mech();
    let mut block = dynamic_bass_block(xmax, re, le, mms, cms, rms, bl, look_ms, rate);
    assert_eq!(block.latency_samples(), look);

    let n = look + 64;
    let mut input = vec![0.0; n];
    // Tiny impulse: well below xmax through the X/V filter.
    input[0] = 1e-6;
    let mut output = vec![0.0; n];
    block.process(&input, &mut output);
    assert!(
        output[look] > 0.0 && output[..look].iter().all(|v| v.abs() < 1e-18),
        "expected delay of {look} samples, got {:?}",
        &output[..=look]
    );
    let h = block.frequency_response(&[100.0], rate)[0];
    let expected = num_complex::Complex64::from_polar(1.0, -2.0 * PI * 100.0 * look as f64 / rate);
    assert!((h - expected).norm() < 1e-9, "H {h} vs {expected}");
}

#[test]
fn loud_bass_tone_is_held_to_xmax() {
    let rate = 48_000.0;
    let (xmax, re, le, mms, cms, rms, bl) = test_woofer_mech();
    let mut block = dynamic_bass_block(xmax, re, le, mms, cms, rms, bl, 5.0, rate);
    // Drive a 40 Hz sine hard enough that free-air peak travel would exceed xmax.
    let freq = 40.0;
    let mag = seatlock_dsp::x_over_v_mag(re, le, mms, cms, rms, bl, freq);
    let volts_peak = (xmax / mag) * 4.0; // 12 dB over the limit
    let n = (rate * 0.25) as usize; // 250 ms
    let mut input = vec![0.0; n];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = volts_peak * (2.0 * PI * freq * i as f64 / rate).sin();
    }
    let mut output = vec![0.0; n];
    let mut offset = 0;
    while offset < n {
        let end = (offset + 128).min(n);
        block.process(&input[offset..end], &mut output[offset..end]);
        offset = end;
    }
    let in_peak = input.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    let out_peak = output
        .iter()
        .skip(n / 2)
        .map(|v| v.abs())
        .fold(0.0_f64, f64::max);
    assert!(
        out_peak < in_peak * 0.6,
        "expected gain reduction: in {in_peak} out {out_peak}"
    );
    let predicted_peak = mag * out_peak;
    assert!(
        predicted_peak <= xmax * 1.25,
        "limited travel {predicted_peak} vs xmax {xmax} (out peak {out_peak})"
    );
}
