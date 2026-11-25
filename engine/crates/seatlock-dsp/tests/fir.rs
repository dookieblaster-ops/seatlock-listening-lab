//! FIR blocks and linear-phase complementary crossovers (README §7).

use seatlock_core::grid::log_grid;
use seatlock_core::registry::DspBlock;
use seatlock_dsp::{crossover_block, fir_block, response_db};

fn run_impulse(block: &mut dyn DspBlock, length: usize) -> Vec<f64> {
    block.reset();
    let mut input = vec![0.0; length];
    input[0] = 1.0;
    let mut output = vec![0.0; length];
    block.process(&input, &mut output);
    output
}

#[test]
fn fir_impulse_matches_reported_response() {
    let rate = 48_000.0;
    // Short symmetric lowpass-ish kernel.
    let taps = vec![0.25, 0.5, 0.25];
    let mut block = fir_block(taps).unwrap();
    assert_eq!(block.latency_samples(), 1);
    let impulse = run_impulse(block.as_mut(), 512);
    let freqs = [100.0, 1_000.0, 5_000.0, 12_000.0];
    for freq in freqs {
        let reported = block.frequency_response(&[freq], rate)[0];
        // DFT of the impulse (length 512).
        let mut re = 0.0;
        let mut im = 0.0;
        for (n, sample) in impulse.iter().enumerate() {
            let phase = -2.0 * std::f64::consts::PI * freq * n as f64 / rate;
            re += sample * phase.cos();
            im += sample * phase.sin();
        }
        let measured = num_complex::Complex64::new(re, im);
        let err_db = 20.0 * ((reported - measured).norm() / reported.norm().max(1e-20)).log10();
        assert!(
            (reported.norm() - measured.norm()).abs() / reported.norm().max(1e-20) < 1e-9,
            "mag at {freq}: reported {} measured {} ({err_db} dB rel)",
            reported.norm(),
            measured.norm()
        );
    }
}

#[test]
fn linear_phase_crossover_sums_to_a_delay_and_reports_latency() {
    let rate = 48_000.0;
    let f_hz = 2_000.0;
    let low = crossover_block("linear-phase", 4, false, f_hz, rate);
    let high = crossover_block("linear-phase", 4, true, f_hz, rate);
    assert_eq!(low.latency_samples(), 64);
    assert_eq!(high.latency_samples(), 64);
    // At the crossover each branch is about −6 dB (amplitude complementary).
    let low_db = response_db(low.as_ref(), f_hz, rate);
    let high_db = response_db(high.as_ref(), f_hz, rate);
    assert!((low_db - -6.02).abs() < 0.3, "lp at fc {low_db}");
    assert!((high_db - -6.02).abs() < 0.3, "hp at fc {high_db}");
    let mut freqs = log_grid(20.0, 20_000.0, 12.0);
    freqs.push(f_hz);
    let delay = low.latency_samples() as f64;
    for freq in freqs {
        let low_h = low.frequency_response(&[freq], rate)[0];
        let high_h = high.frequency_response(&[freq], rate)[0];
        let sum = low_h + high_h;
        let want = num_complex::Complex64::from_polar(
            1.0,
            -2.0 * std::f64::consts::PI * freq * delay / rate,
        );
        let err = (sum - want).norm();
        assert!(
            err < 1e-9,
            "linear-phase sum at {freq} Hz off by {err} (got {sum}, want {want})"
        );
    }
}

#[test]
fn empty_fir_is_refused() {
    assert!(fir_block(vec![]).is_err());
}
