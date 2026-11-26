//! C6 look-ahead limiter driven by cone travel `X/V` (README §8).
//!
//! Continuous model (free-air load): `X/V = bl / (s (Ze Zm + bl²))` with
//! `Ze = Re + s Le`, `Zm = Rms + s Mms + 1/(s Cms)`. Discretized by bilinear
//! transform. Small signals pass as a pure look-ahead delay; large bass is
//! gain-reduced so predicted peak |x| stays at `xmax_m`.

use num_complex::Complex64;
use seatlock_core::registry::DspBlock;
use serde_json::{Map, Value};
use std::f64::consts::PI;

/// Look-ahead dynamic bass limiter.
pub struct DynamicBass {
    xmax_m: f64,
    look_ahead: usize,
    delay: Vec<f64>,
    delay_pos: usize,
    /// Sidechain peak history of |x| over the look-ahead window.
    peak_hist: Vec<f64>,
    peak_pos: usize,
    peak_max: f64,
    /// Direct-form IIR for X/V (order 2 or 3). `b` numerator, `a` denominator with a[0]=1.
    b: [f64; 4],
    a: [f64; 4],
    order: usize,
    x_hist: [f64; 4],
    y_hist: [f64; 4],
    gain: f64,
    attack: f64,
    release: f64,
}

impl DynamicBass {
    fn predict_x(&mut self, voltage: f64) -> f64 {
        let n = self.order;
        let mut acc = self.b[0] * voltage;
        for k in 1..=n {
            acc += self.b[k] * self.x_hist[k - 1] - self.a[k] * self.y_hist[k - 1];
        }
        for k in (1..n).rev() {
            self.x_hist[k] = self.x_hist[k - 1];
            self.y_hist[k] = self.y_hist[k - 1];
        }
        if n > 0 {
            self.x_hist[0] = voltage;
            self.y_hist[0] = acc;
        }
        acc
    }

    fn update_peak(&mut self, abs_x: f64) {
        let old = self.peak_hist[self.peak_pos];
        self.peak_hist[self.peak_pos] = abs_x;
        self.peak_pos = (self.peak_pos + 1) % self.peak_hist.len();
        if abs_x >= self.peak_max {
            self.peak_max = abs_x;
        } else if (old - self.peak_max).abs() <= 1e-18 * (1.0 + self.peak_max) {
            self.peak_max = self.peak_hist.iter().copied().fold(0.0_f64, f64::max);
        }
    }
}

impl DspBlock for DynamicBass {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        // Linear small-signal path is a pure look-ahead delay.
        let samples = self.look_ahead as f64;
        freqs_hz
            .iter()
            .map(|freq| Complex64::from_polar(1.0, -2.0 * PI * freq * samples / sample_rate_hz))
            .collect()
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        if self.look_ahead == 0 {
            for (sample, slot) in input.iter().zip(output.iter_mut()) {
                let x = self.predict_x(*sample);
                self.update_peak(x.abs());
                let target = if self.peak_max > self.xmax_m && self.peak_max > 1e-18 {
                    self.xmax_m / self.peak_max
                } else {
                    1.0
                };
                let coeff = if target < self.gain {
                    self.attack
                } else {
                    self.release
                };
                self.gain += coeff * (target - self.gain);
                *slot = sample * self.gain;
            }
            return;
        }
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            let delayed = self.delay[self.delay_pos];
            self.delay[self.delay_pos] = *sample;
            self.delay_pos = (self.delay_pos + 1) % self.look_ahead;

            let x = self.predict_x(*sample);
            self.update_peak(x.abs());
            let target = if self.peak_max > self.xmax_m && self.peak_max > 1e-18 {
                self.xmax_m / self.peak_max
            } else {
                1.0
            };
            let coeff = if target < self.gain {
                self.attack
            } else {
                self.release
            };
            self.gain += coeff * (target - self.gain);
            *slot = delayed * self.gain;
        }
    }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.delay_pos = 0;
        self.peak_hist.fill(0.0);
        self.peak_pos = 0;
        self.peak_max = 0.0;
        self.x_hist = [0.0; 4];
        self.y_hist = [0.0; 4];
        self.gain = 1.0;
    }

    fn latency_samples(&self) -> usize {
        self.look_ahead
    }
}

/// Continuous-time |X/V| magnitude at `freq_hz` (free air): `|bl / (jω (Ze Zm + bl²))|`.
pub fn x_over_v_mag(
    re_ohm: f64,
    le_h: f64,
    mms_kg: f64,
    cms_m_per_n: f64,
    rms_ns_per_m: f64,
    bl_tm: f64,
    freq_hz: f64,
) -> f64 {
    let omega = 2.0 * PI * freq_hz;
    let s = Complex64::new(0.0, omega);
    let ze = Complex64::new(re_ohm, omega * le_h);
    let zm = Complex64::new(rms_ns_per_m, 0.0) + s * mms_kg + 1.0 / (s * cms_m_per_n);
    let denom = s * (ze * zm + bl_tm * bl_tm);
    if denom.norm() < 1e-30 {
        return 0.0;
    }
    (bl_tm / denom).norm()
}

fn bilinear_xv(
    re_ohm: f64,
    le_h: f64,
    mms_kg: f64,
    cms_m_per_n: f64,
    rms_ns_per_m: f64,
    bl_tm: f64,
    sample_rate_hz: f64,
) -> (usize, [f64; 4], [f64; 4]) {
    // H(s) = bl / (a3 s³ + a2 s² + a1 s + a0)
    let a3 = le_h * mms_kg;
    let a2 = re_ohm * mms_kg + le_h * rms_ns_per_m;
    let a1 = re_ohm * rms_ns_per_m + bl_tm * bl_tm + le_h / cms_m_per_n;
    let a0 = re_ohm / cms_m_per_n;
    let k = 2.0 * sample_rate_hz;

    if a3 < 1e-18 {
        // 2nd order: H(s) = bl / (a2 s² + a1 s + a0)
        // s = K (1-z)/(1+z); multiply num/den by (1+z)²
        let b0 = bl_tm;
        let n0 = b0;
        let n1 = 2.0 * b0;
        let n2 = b0;
        let d0 = a2 * k * k + a1 * k + a0;
        let d1 = -2.0 * a2 * k * k + 2.0 * a0;
        let d2 = a2 * k * k - a1 * k + a0;
        let inv = 1.0 / d0;
        return (
            2,
            [n0 * inv, n1 * inv, n2 * inv, 0.0],
            [1.0, d1 * inv, d2 * inv, 0.0],
        );
    }

    // 3rd order: multiply by (1+z)³
    let b0 = bl_tm;
    let n0 = b0;
    let n1 = 3.0 * b0;
    let n2 = 3.0 * b0;
    let n3 = b0;
    let d0 = a3 * k.powi(3) + a2 * k * k + a1 * k + a0;
    let d1 = -3.0 * a3 * k.powi(3) - a2 * k * k + a1 * k + 3.0 * a0;
    let d2 = 3.0 * a3 * k.powi(3) - a2 * k * k - a1 * k + 3.0 * a0;
    let d3 = -a3 * k.powi(3) + a2 * k * k - a1 * k + a0;
    let inv = 1.0 / d0;
    (
        3,
        [n0 * inv, n1 * inv, n2 * inv, n3 * inv],
        [1.0, d1 * inv, d2 * inv, d3 * inv],
    )
}

pub fn parse_dynamic_bass(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let xmax_m = fields
        .get("xmax_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs xmax_m".to_string())?;
    if xmax_m <= 0.0 {
        return Err("dynamic_bass xmax_m must be positive".into());
    }
    let re_ohm = fields
        .get("re_ohm")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs re_ohm".to_string())?;
    let le_h = fields.get("le_h").and_then(Value::as_f64).unwrap_or(0.0);
    let mms_kg = fields
        .get("mms_kg")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs mms_kg".to_string())?;
    let cms_m_per_n = fields
        .get("cms_m_per_n")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs cms_m_per_n".to_string())?;
    let rms_ns_per_m = fields
        .get("rms_ns_per_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs rms_ns_per_m".to_string())?;
    let bl_tm = fields
        .get("bl_tm")
        .and_then(Value::as_f64)
        .ok_or_else(|| "dynamic_bass needs bl_tm".to_string())?;
    let sample_rate_hz = fields
        .get("sample_rate_hz")
        .and_then(Value::as_f64)
        .unwrap_or(48_000.0)
        .max(1.0);
    let look_ahead_ms = fields
        .get("look_ahead_ms")
        .and_then(Value::as_f64)
        .unwrap_or(5.0)
        .max(0.0);
    let look_ahead = (look_ahead_ms * 1e-3 * sample_rate_hz).round().max(0.0) as usize;
    // ~0.5 ms attack, ~50 ms release at this rate.
    let attack = 1.0 - (-1.0 / (0.0005 * sample_rate_hz).max(1.0)).exp();
    let release = 1.0 - (-1.0 / (0.05 * sample_rate_hz).max(1.0)).exp();
    let (order, b, a) = bilinear_xv(
        re_ohm,
        le_h,
        mms_kg,
        cms_m_per_n,
        rms_ns_per_m,
        bl_tm,
        sample_rate_hz,
    );
    Ok(Box::new(DynamicBass {
        xmax_m,
        look_ahead,
        delay: vec![0.0; look_ahead.max(1)],
        delay_pos: 0,
        peak_hist: vec![0.0; look_ahead.max(1)],
        peak_pos: 0,
        peak_max: 0.0,
        b,
        a,
        order,
        x_hist: [0.0; 4],
        y_hist: [0.0; 4],
        gain: 1.0,
        attack,
        release,
    }))
}

/// Build a dynamic-bass block from mechanical params (tests / helpers).
#[allow(clippy::too_many_arguments)]
pub fn dynamic_bass_block(
    xmax_m: f64,
    re_ohm: f64,
    le_h: f64,
    mms_kg: f64,
    cms_m_per_n: f64,
    rms_ns_per_m: f64,
    bl_tm: f64,
    look_ahead_ms: f64,
    sample_rate_hz: f64,
) -> Box<dyn DspBlock> {
    let mut fields = Map::new();
    fields.insert("xmax_m".into(), Value::from(xmax_m));
    fields.insert("re_ohm".into(), Value::from(re_ohm));
    fields.insert("le_h".into(), Value::from(le_h));
    fields.insert("mms_kg".into(), Value::from(mms_kg));
    fields.insert("cms_m_per_n".into(), Value::from(cms_m_per_n));
    fields.insert("rms_ns_per_m".into(), Value::from(rms_ns_per_m));
    fields.insert("bl_tm".into(), Value::from(bl_tm));
    fields.insert("look_ahead_ms".into(), Value::from(look_ahead_ms));
    fields.insert("sample_rate_hz".into(), Value::from(sample_rate_hz));
    parse_dynamic_bass(&fields).expect("dynamic_bass")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_matches_continuous_x_over_v_in_the_bass() {
        // test-woofer-a derived mech (approximate from published).
        let re = 6.0;
        let le = 0.0005;
        let fs = 50.0;
        let qms = 4.0;
        let qes = 0.45;
        let vas = 0.01;
        let sd = 0.0085;
        let rho = 1.204;
        let c0 = 343.2;
        let cms = vas / (rho * c0 * c0 * sd * sd);
        let omega_s = 2.0 * PI * fs;
        let mms = 1.0 / (omega_s * omega_s * cms);
        let rms = omega_s * mms / qms;
        let bl = (omega_s * mms * re / qes).sqrt();
        let rate = 48_000.0;
        let (order, b, a) = bilinear_xv(re, le, mms, cms, rms, bl, rate);
        assert!(order >= 2);
        for freq in [20.0, 40.0, 80.0, 160.0, 320.0] {
            let continuous = x_over_v_mag(re, le, mms, cms, rms, bl, freq);
            let z = Complex64::from_polar(1.0, -2.0 * PI * freq / rate);
            let mut num = Complex64::new(0.0, 0.0);
            let mut den = Complex64::new(0.0, 0.0);
            let mut zk = Complex64::new(1.0, 0.0);
            for k in 0..=order {
                num += b[k] * zk;
                den += a[k] * zk;
                zk *= z;
            }
            let discrete = (num / den).norm();
            let rel = (discrete - continuous).abs() / continuous.max(1e-12);
            assert!(
                rel < 0.08,
                "{freq} Hz: discrete {discrete} vs continuous {continuous} (rel {rel})"
            );
        }
    }
}
