//! FIR blocks and complementary linear-phase crossovers (README §7 Level 3).

use num_complex::Complex64;
use seatlock_core::registry::DspBlock;
use serde_json::{Map, Value};
use std::f64::consts::PI;

/// Causal FIR with an explicit impulse response (`taps`).
pub struct Fir {
    taps: Vec<f64>,
    /// Group delay in samples for a linear-phase (symmetric) FIR: `(N - 1) / 2`.
    latency: usize,
    delay: Vec<f64>,
    pos: usize,
}

impl Fir {
    pub fn from_taps(taps: Vec<f64>) -> Result<Self, String> {
        if taps.is_empty() {
            return Err("an FIR needs at least one tap.".into());
        }
        if taps.iter().any(|t| !t.is_finite()) {
            return Err("FIR taps must be finite numbers.".into());
        }
        let n = taps.len();
        let latency = if n > 1 && is_symmetric(&taps) {
            (n - 1) / 2
        } else {
            // Causal FIR of length N delays by up to N-1; report the centre for odd
            // lengths even when asymmetric, else N-1.
            if n % 2 == 1 {
                (n - 1) / 2
            } else {
                n.saturating_sub(1)
            }
        };
        Ok(Self {
            delay: vec![0.0; n],
            taps,
            latency,
            pos: 0,
        })
    }
}

fn is_symmetric(taps: &[f64]) -> bool {
    let n = taps.len();
    (0..n / 2).all(|i| (taps[i] - taps[n - 1 - i]).abs() <= 1e-12 * (1.0 + taps[i].abs()))
}

impl DspBlock for Fir {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .map(|freq| {
                let mut h = Complex64::new(0.0, 0.0);
                for (n, tap) in self.taps.iter().enumerate() {
                    let phase = -2.0 * PI * freq * n as f64 / sample_rate_hz;
                    h += Complex64::from_polar(*tap, phase);
                }
                h
            })
            .collect()
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        let n = self.taps.len();
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            self.delay[self.pos] = *sample;
            let mut y = 0.0;
            let mut idx = self.pos;
            for tap in &self.taps {
                y += tap * self.delay[idx];
                idx = if idx == 0 { n - 1 } else { idx - 1 };
            }
            *slot = y;
            self.pos = (self.pos + 1) % n;
        }
    }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.pos = 0;
    }

    fn latency_samples(&self) -> usize {
        self.latency
    }
}

/// Odd tap count for a linear-phase crossover of Linkwitz–Riley-like steepness.
fn linear_phase_length(order: usize) -> Result<usize, String> {
    let length = match order {
        2 => 65,
        4 => 129,
        8 => 257,
        _ => {
            return Err(
                "linear-phase crossover order must be 2, 4, or 8 (tap counts 65 / 129 / 257)."
                    .into(),
            )
        }
    };
    Ok(length)
}

/// Hann-windowed sinc lowpass, odd length, unity DC gain. Complementary highpass is
/// `δ[delay] − lowpass`, so low + high is a pure delay of `(N−1)/2` samples.
pub fn linear_phase_crossover_taps(
    order: usize,
    highpass: bool,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> Result<Vec<f64>, String> {
    if !(freq_hz > 0.0 && freq_hz < sample_rate_hz * 0.5) {
        return Err(format!(
            "linear-phase crossover frequency {freq_hz} Hz is outside 0 to Nyquist."
        ));
    }
    let n = linear_phase_length(order)?;
    let delay = (n - 1) / 2;
    let fc = freq_hz / sample_rate_hz;
    let mut taps = vec![0.0; n];
    for (i, tap) in taps.iter_mut().enumerate() {
        let m = i as f64 - delay as f64;
        let sinc = if m.abs() < 1e-12 {
            2.0 * fc
        } else {
            (2.0 * PI * fc * m).sin() / (PI * m)
        };
        let window = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n - 1) as f64).cos());
        *tap = sinc * window;
    }
    let dc: f64 = taps.iter().sum();
    if dc.abs() < 1e-18 {
        return Err("linear-phase lowpass DC gain vanished.".into());
    }
    for tap in &mut taps {
        *tap /= dc;
    }
    if highpass {
        let mut high = vec![0.0; n];
        high[delay] = 1.0;
        for (h, l) in high.iter_mut().zip(&taps) {
            *h -= *l;
        }
        Ok(high)
    } else {
        Ok(taps)
    }
}

pub fn parse_fir(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let taps = fields
        .get("taps")
        .and_then(Value::as_array)
        .ok_or("missing taps")?;
    let coeffs: Result<Vec<f64>, String> = taps
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_f64()
                .ok_or_else(|| format!("FIR tap {i} is not a number."))
        })
        .collect();
    Ok(Box::new(Fir::from_taps(coeffs?)?))
}

pub fn fir_block(taps: Vec<f64>) -> Result<Box<dyn DspBlock>, String> {
    Ok(Box::new(Fir::from_taps(taps)?))
}

pub fn linear_phase_crossover_block(
    order: usize,
    highpass: bool,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> Result<Box<dyn DspBlock>, String> {
    let taps = linear_phase_crossover_taps(order, highpass, freq_hz, sample_rate_hz)?;
    fir_block(taps)
}
