//! Filters, crossover blocks, delay, gain, limiter, and their frequency responses.

use num_complex::Complex64;
use seatlock_core::registry::{DspBlock, Registry};
use serde_json::{Map, Value};
use std::f64::consts::PI;

pub fn register(registry: &mut Registry) {
    registry.register_dsp("biquad", parse_biquad);
    registry.register_dsp("gain", parse_gain);
    registry.register_dsp("delay", parse_delay);
    registry.register_dsp("polarity", parse_polarity);
}

#[derive(Clone)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Biquad {
    fn from_coeffs(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }
}

impl DspBlock for Biquad {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .map(|freq| {
                let zm1 = Complex64::from_polar(1.0, -2.0 * PI * freq / sample_rate_hz);
                let zm2 = zm1 * zm1;
                (self.b0 + self.b1 * zm1 + self.b2 * zm2)
                    / (Complex64::new(1.0, 0.0) + self.a1 * zm1 + self.a2 * zm2)
            })
            .collect()
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            *slot = biquad_step(
                self.b0,
                self.b1,
                self.b2,
                self.a1,
                self.a2,
                &mut self.s1,
                &mut self.s2,
                *sample,
            );
        }
    }

    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

fn rbj(
    kind: &str,
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    sample_rate_hz: f64,
) -> Result<Biquad, String> {
    let w0 = 2.0 * PI * freq_hz / sample_rate_hz;
    let cos = w0.cos();
    let sin = w0.sin();
    let alpha = sin / (2.0 * q);
    let a = 10_f64.powf(gain_db / 40.0);
    let (b0, b1, b2, a0, a1, a2) = match kind {
        "lowpass" => (
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        "highpass" => (
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        "peaking" => (
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        ),
        "notch" => (1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
        "allpass" => (
            1.0 - alpha,
            -2.0 * cos,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ),
        "lowshelf" | "highshelf" => {
            let slope = q;
            let shelf_alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
            let sqrt_a = a.sqrt();
            if kind == "lowshelf" {
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha),
                    (a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha,
                )
            } else {
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha),
                    (a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * shelf_alpha,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * shelf_alpha,
                )
            }
        }
        other => return Err(format!("unknown biquad type {other}")),
    };
    Ok(Biquad::from_coeffs(b0, b1, b2, a0, a1, a2))
}

fn make_biquad(
    kind: &str,
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    from_hz: f64,
    from_q: f64,
    sample_rate_hz: f64,
) -> Result<Biquad, String> {
    if kind == "linkwitz" {
        let w0 = 2.0 * sample_rate_hz * (std::f64::consts::PI * from_hz / sample_rate_hz).tan();
        let w1 = 2.0 * sample_rate_hz * (std::f64::consts::PI * freq_hz / sample_rate_hz).tan();
        let c = 2.0 * sample_rate_hz;
        let coef = |omega: f64, quality: f64| {
            (
                c * c + (omega / quality) * c + omega * omega,
                -2.0 * c * c + 2.0 * omega * omega,
                c * c - (omega / quality) * c + omega * omega,
            )
        };
        let (b0, b1, b2) = coef(w0, from_q.max(0.2));
        let (a0, a1, a2) = coef(w1, q.max(0.2));
        return Ok(Biquad::from_coeffs(b0, b1, b2, a0, a1, a2));
    }
    rbj(kind, freq_hz, q, gain_db, sample_rate_hz)
}

fn parse_biquad(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let kind = fields
        .get("type")
        .and_then(Value::as_str)
        .ok_or("missing type")?;
    let freq = fields
        .get("f_hz")
        .and_then(Value::as_f64)
        .ok_or("missing f_hz")?;
    let q = fields.get("q").and_then(Value::as_f64).unwrap_or(0.707);
    let gain_db = fields.get("gain_db").and_then(Value::as_f64).unwrap_or(0.0);
    // Coefficients are rebuilt per sample rate in a wrapper so one block serves every rate.
    Ok(Box::new(RateBiquad {
        kind: kind.to_string(),
        freq_hz: freq,
        q,
        gain_db,
        from_hz: fields
            .get("from_hz")
            .and_then(Value::as_f64)
            .unwrap_or(freq),
        from_q: fields.get("from_q").and_then(Value::as_f64).unwrap_or(q),
        state: None,
    }))
}

struct RateBiquad {
    kind: String,
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    from_hz: f64,
    from_q: f64,
    state: Option<(f64, Biquad)>,
}

impl RateBiquad {
    fn biquad(&mut self, sample_rate_hz: f64) -> Result<&mut Biquad, String> {
        let rebuild = match &self.state {
            Some((rate, _)) => (*rate - sample_rate_hz).abs() > 1e-9,
            None => true,
        };
        if rebuild {
            self.state = Some((
                sample_rate_hz,
                make_biquad(
                    &self.kind,
                    self.freq_hz,
                    self.q,
                    self.gain_db,
                    self.from_hz,
                    self.from_q,
                    sample_rate_hz,
                )?,
            ));
        }
        Ok(&mut self.state.as_mut().unwrap().1)
    }
}

impl DspBlock for RateBiquad {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        let biquad = make_biquad(
            &self.kind,
            self.freq_hz,
            self.q,
            self.gain_db,
            self.from_hz,
            self.from_q,
            sample_rate_hz,
        )
        .expect("biquad");
        biquad.frequency_response(freqs_hz, sample_rate_hz)
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        let biquad = self.biquad(48_000.0).expect("biquad");
        biquad.process(input, output);
    }

    fn reset(&mut self) {
        if let Some((_, biquad)) = self.state.as_mut() {
            biquad.reset();
        }
    }
}

struct Gain {
    linear: f64,
}

impl DspBlock for Gain {
    fn frequency_response(&self, freqs_hz: &[f64], _sample_rate_hz: f64) -> Vec<Complex64> {
        vec![Complex64::new(self.linear, 0.0); freqs_hz.len()]
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            *slot = sample * self.linear;
        }
    }

    fn reset(&mut self) {}
}

fn parse_gain(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let gain_db = fields.get("gain_db").and_then(Value::as_f64).unwrap_or(0.0);
    Ok(Box::new(Gain {
        linear: 10_f64.powf(gain_db / 20.0),
    }))
}

struct Delay {
    samples: usize,
    buffer: Vec<f64>,
    pos: usize,
}

impl DspBlock for Delay {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .map(|freq| {
                Complex64::from_polar(1.0, -2.0 * PI * freq * self.samples as f64 / sample_rate_hz)
            })
            .collect()
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        if self.samples == 0 {
            output[..input.len()].copy_from_slice(input);
            return;
        }
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            *slot = self.buffer[self.pos];
            self.buffer[self.pos] = *sample;
            self.pos = (self.pos + 1) % self.samples;
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}

fn delay_samples(fields: &Map<String, Value>, sample_rate_hz: f64) -> usize {
    if let Some(samples) = fields.get("samples").and_then(Value::as_u64) {
        return samples as usize;
    }
    let seconds = fields.get("delay_s").and_then(Value::as_f64).unwrap_or(0.0);
    (seconds * sample_rate_hz).round().max(0.0) as usize
}

fn parse_delay(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let samples = delay_samples(fields, 48_000.0);
    Ok(Box::new(Delay {
        samples,
        buffer: vec![0.0; samples.max(1)],
        pos: 0,
    }))
}

pub fn delay_block(delay_s: f64, sample_rate_hz: f64) -> Box<dyn DspBlock> {
    let mut fields = Map::new();
    fields.insert("delay_s".into(), Value::from(delay_s));
    let samples = delay_samples(&fields, sample_rate_hz);
    Box::new(Delay {
        samples,
        buffer: vec![0.0; samples.max(1)],
        pos: 0,
    })
}

pub fn gain_block(gain_db: f64) -> Box<dyn DspBlock> {
    Box::new(Gain {
        linear: 10_f64.powf(gain_db / 20.0),
    })
}

pub fn polarity_block(invert: bool) -> Box<dyn DspBlock> {
    Box::new(Polarity {
        sign: if invert { -1.0 } else { 1.0 },
    })
}

struct Polarity {
    sign: f64,
}

impl DspBlock for Polarity {
    fn frequency_response(&self, freqs_hz: &[f64], _sample_rate_hz: f64) -> Vec<Complex64> {
        vec![Complex64::new(self.sign, 0.0); freqs_hz.len()]
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            *slot = sample * self.sign;
        }
    }

    fn reset(&mut self) {}
}

fn parse_polarity(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let invert = fields
        .get("invert")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Ok(polarity_block(invert))
}

/// One sample of the direct-form biquad. The real-time kernel calls this, and so does `process`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn biquad_step(
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: &mut f64,
    s2: &mut f64,
    x: f64,
) -> f64 {
    let y = b0 * x + *s1;
    *s1 = b1 * x - a1 * y + *s2;
    *s2 = b2 * x - a2 * y;
    y
}

/// Normalized coefficients `(b0, b1, b2, a1, a2)` for a biquad at `sample_rate_hz`.
pub fn biquad_coefficients(
    kind: &str,
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    from_hz: f64,
    from_q: f64,
    sample_rate_hz: f64,
) -> Result<(f64, f64, f64, f64, f64), String> {
    let filter = make_biquad(kind, freq_hz, q, gain_db, from_hz, from_q, sample_rate_hz)?;
    Ok((filter.b0, filter.b1, filter.b2, filter.a1, filter.a2))
}

/// Biquad used by CHK-DSP. The sample rate is fixed so process and the response share one filter.
pub fn biquad_block(
    kind: &str,
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    sample_rate_hz: f64,
) -> Box<dyn DspBlock> {
    Box::new(rbj(kind, freq_hz, q, gain_db, sample_rate_hz).expect("biquad"))
}

pub fn response_db(block: &dyn DspBlock, freq_hz: f64, sample_rate_hz: f64) -> f64 {
    let response = block.frequency_response(&[freq_hz], sample_rate_hz)[0];
    20.0 * response.norm().log10()
}
