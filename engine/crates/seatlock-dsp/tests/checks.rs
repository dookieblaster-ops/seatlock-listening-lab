use seatlock_core::grid::log_grid;
use seatlock_core::registry::DspBlock;
use seatlock_dsp::{biquad_block, crossover_block, response_db};
use std::f64::consts::PI;

fn run_impulse(block: &mut dyn DspBlock, length: usize, chunk: usize) -> Vec<f64> {
    block.reset();
    let mut input = vec![0.0; length];
    input[0] = 1.0;
    let mut output = vec![0.0; length];
    let mut offset = 0;
    while offset < length {
        let end = (offset + chunk).min(length);
        block.process(&input[offset..end], &mut output[offset..end]);
        offset = end;
    }
    output
}

fn fft_matches(block: &dyn DspBlock, impulse: &[f64], sample_rate: f64) {
    let n = impulse.len();
    let mut re = impulse.to_vec();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    for k in 0..=n / 2 {
        let freq = k as f64 * sample_rate / n as f64;
        let reported = block.frequency_response(&[freq], sample_rate)[0];
        let measured = num_complex::Complex64::new(re[k], im[k]);
        let reported_db = 20.0 * reported.norm().max(1e-30).log10();
        let measured_db = 20.0 * measured.norm().max(1e-30).log10();
        assert!(
            (reported_db - measured_db).abs() <= 0.01,
            "bin {k} mag {measured_db} vs {reported_db}"
        );
        let mut phase = (measured.arg() - reported.arg()).to_degrees();
        while phase > 180.0 {
            phase -= 360.0;
        }
        while phase < -180.0 {
            phase += 360.0;
        }
        assert!(phase.abs() <= 0.1, "bin {k} phase {phase}");
    }
}

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f64;
        let wlen_re = angle.cos();
        let wlen_im = angle.sin();
        let mut i = 0;
        while i < n {
            let mut w_re = 1.0;
            let mut w_im = 0.0;
            for k in 0..len / 2 {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let v_im = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + len / 2] = u_re - v_re;
                im[i + k + len / 2] = u_im - v_im;
                let next_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = next_re;
            }
            i += len;
        }
        len <<= 1;
    }
}

#[test]
#[allow(clippy::approx_constant)]
fn chk_dsp_1_lowpass_and_peaking() {
    let lowpass = biquad_block("lowpass", 1_000.0, 0.7071, 0.0, 48_000.0); // CHK-DSP-1 prints Q as 0.7071
    let peaking = biquad_block("peaking", 1_000.0, 2.0, 6.0, 48_000.0);
    let low_db = response_db(lowpass.as_ref(), 1_000.0, 48_000.0);
    let peak_db = response_db(peaking.as_ref(), 1_000.0, 48_000.0);
    assert!((low_db - -3.01).abs() <= 0.01, "lowpass {low_db}");
    assert!((peak_db - 6.0).abs() <= 0.01, "peaking {peak_db}");
}

#[test]
#[allow(clippy::approx_constant)]
fn chk_dsp_3_impulse_matches_response_for_every_block_size() {
    let sample_rate = 48_000.0;
    let length = 8192;
    let mut reference = biquad_block("lowpass", 1_000.0, 0.7071, 0.0, sample_rate);
    let by_one = run_impulse(reference.as_mut(), length, 1);
    let by_128 = run_impulse(reference.as_mut(), length, 128);
    let by_4096 = run_impulse(reference.as_mut(), length, 4096);
    assert_eq!(by_one, by_128);
    assert_eq!(by_one, by_4096);
    fft_matches(reference.as_ref(), &by_one, sample_rate);
}

#[test]
fn chk_dsp_2_linkwitz_riley_sums() {
    let rate = 48_000.0;
    let low = crossover_block("linkwitz-riley", 4, false, 2_000.0, rate);
    let high = crossover_block("linkwitz-riley", 4, true, 2_000.0, rate);
    let low_db = response_db(low.as_ref(), 2_000.0, rate);
    let high_db = response_db(high.as_ref(), 2_000.0, rate);
    assert!((low_db - -6.02).abs() <= 0.02, "lr4 low {low_db}");
    assert!((high_db - -6.02).abs() <= 0.02, "lr4 high {high_db}");
    let mut freqs = log_grid(20.0, 20_000.0, 12.0);
    freqs.push(2_000.0);
    for freq in freqs {
        let low_h = low.frequency_response(&[freq], rate)[0];
        let high_h = high.frequency_response(&[freq], rate)[0];
        let sum_db = 20.0 * (low_h + high_h).norm().max(1e-20).log10();
        assert!(sum_db.abs() <= 0.001, "lr4 sum at {freq} is {sum_db}");
    }
    let lr2_low = crossover_block("linkwitz-riley", 2, false, 2_000.0, rate);
    let lr2_high = crossover_block("linkwitz-riley", 2, true, 2_000.0, rate);
    let low_h = lr2_low.frequency_response(&[2_000.0], rate)[0];
    let high_h = lr2_high.frequency_response(&[2_000.0], rate)[0];
    let in_phase = 20.0 * (low_h + high_h).norm().max(1e-20).log10();
    let flipped = 20.0 * (low_h - high_h).norm().max(1e-20).log10();
    assert!(in_phase < -40.0, "lr2 in phase at 2 kHz is {in_phase}");
    assert!(flipped.abs() <= 0.001, "lr2 flipped at 2 kHz is {flipped}");
    for freq in [100.0, 500.0, 2_000.0, 8_000.0, 16_000.0] {
        let left = lr2_low.frequency_response(&[freq], rate)[0];
        let right = lr2_high.frequency_response(&[freq], rate)[0];
        let sum_db = 20.0 * (left - right).norm().max(1e-20).log10();
        assert!(
            sum_db.abs() <= 0.001,
            "lr2 flipped sum at {freq} is {sum_db}"
        );
    }
}
