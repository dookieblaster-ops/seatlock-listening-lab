//! Butterworth and Linkwitz–Riley crossovers, as cascades of biquads.

use num_complex::Complex64;
use seatlock_core::registry::DspBlock;
use serde_json::{Map, Value};
use std::f64::consts::PI;

use crate::biquad_step;

struct Section {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

pub struct Crossover {
    sections: Vec<Section>,
}

impl Crossover {
    pub fn new(
        family: &str,
        order: usize,
        highpass: bool,
        freq_hz: f64,
        sample_rate_hz: f64,
    ) -> Result<Self, String> {
        let sections = match family {
            "butterworth" => butterworth_sections(order, highpass, freq_hz, sample_rate_hz)?,
            "linkwitz-riley" | "lr" => {
                let half = match order {
                    2 => 1,
                    4 => 2,
                    8 => 4,
                    _ => return Err("Linkwitz-Riley order must be 2, 4, or 8".into()),
                };
                let mut sections = butterworth_sections(half, highpass, freq_hz, sample_rate_hz)?;
                let again = butterworth_sections(half, highpass, freq_hz, sample_rate_hz)?;
                sections.extend(again);
                sections
            }
            "linear-phase" | "fir" => {
                return Err(
                    "linear-phase crossovers are FIR blocks; use family linear-phase through \
                     parse_crossover, which returns an FIR."
                        .into(),
                );
            }
            other => return Err(format!("unknown crossover family {other}")),
        };
        Ok(Self { sections })
    }
}

fn butterworth_sections(
    order: usize,
    highpass: bool,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> Result<Vec<Section>, String> {
    if !(1..=4).contains(&order) {
        return Err("Butterworth order must be 1 to 4".into());
    }
    let mut sections = Vec::new();
    if order % 2 == 1 {
        sections.push(first_order(highpass, freq_hz, sample_rate_hz));
    }
    let pairs = order / 2;
    for index in 0..pairs {
        let k = index as f64;
        let n = order as f64;
        let q = 1.0 / (2.0 * ((2.0 * k + 1.0) * PI / (2.0 * n)).sin());
        sections.push(second_order(highpass, freq_hz, q, sample_rate_hz)?);
    }
    Ok(sections)
}

fn first_order(highpass: bool, freq_hz: f64, sample_rate_hz: f64) -> Section {
    let k = (PI * freq_hz / sample_rate_hz).tan();
    let norm = 1.0 + k;
    let (b0, b1) = if highpass {
        (1.0 / norm, -1.0 / norm)
    } else {
        (k / norm, k / norm)
    };
    Section {
        b0,
        b1,
        b2: 0.0,
        a1: (k - 1.0) / norm,
        a2: 0.0,
        s1: 0.0,
        s2: 0.0,
    }
}

fn second_order(
    highpass: bool,
    freq_hz: f64,
    q: f64,
    sample_rate_hz: f64,
) -> Result<Section, String> {
    let kind = if highpass { "highpass" } else { "lowpass" };
    let (b0, b1, b2, a1, a2) =
        crate::biquad_coefficients(kind, freq_hz, q, 0.0, freq_hz, q, sample_rate_hz)?;
    Ok(Section {
        b0,
        b1,
        b2,
        a1,
        a2,
        s1: 0.0,
        s2: 0.0,
    })
}

impl DspBlock for Crossover {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .map(|freq| {
                let mut total = Complex64::new(1.0, 0.0);
                let zm1 = Complex64::from_polar(1.0, -2.0 * PI * freq / sample_rate_hz);
                let zm2 = zm1 * zm1;
                for section in &self.sections {
                    let num = section.b0 + section.b1 * zm1 + section.b2 * zm2;
                    let den = Complex64::new(1.0, 0.0) + section.a1 * zm1 + section.a2 * zm2;
                    total *= num / den;
                }
                total
            })
            .collect()
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        for (sample, slot) in input.iter().zip(output.iter_mut()) {
            let mut value = *sample;
            for section in &mut self.sections {
                value = biquad_step(
                    section.b0,
                    section.b1,
                    section.b2,
                    section.a1,
                    section.a2,
                    &mut section.s1,
                    &mut section.s2,
                    value,
                );
            }
            *slot = value;
        }
    }

    fn reset(&mut self) {
        for section in &mut self.sections {
            section.s1 = 0.0;
            section.s2 = 0.0;
        }
    }
}

pub fn parse_crossover(fields: &Map<String, Value>) -> Result<Box<dyn DspBlock>, String> {
    let family = fields
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or("linkwitz-riley");
    let order = fields.get("order").and_then(Value::as_u64).unwrap_or(4) as usize;
    let freq = fields
        .get("f_hz")
        .and_then(Value::as_f64)
        .ok_or("missing f_hz")?;
    let band = fields.get("band").and_then(Value::as_str).unwrap_or("low");
    let rate = fields
        .get("sample_rate_hz")
        .and_then(Value::as_f64)
        .unwrap_or(48_000.0);
    if family == "linear-phase" || family == "fir" {
        return crate::fir::linear_phase_crossover_block(order, band == "high", freq, rate);
    }
    Ok(Box::new(Crossover::new(
        family,
        order,
        band == "high",
        freq,
        rate,
    )?))
}

pub fn crossover_block(
    family: &str,
    order: usize,
    highpass: bool,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> Box<dyn DspBlock> {
    if family == "linear-phase" || family == "fir" {
        return crate::fir::linear_phase_crossover_block(order, highpass, freq_hz, sample_rate_hz)
            .expect("linear-phase crossover");
    }
    Box::new(Crossover::new(family, order, highpass, freq_hz, sample_rate_hz).expect("crossover"))
}

/// Biquad coefficients for an IIR crossover (Butterworth / Linkwitz–Riley) at `sample_rate_hz`.
/// Each row is `(b0, b1, b2, a1, a2)`. Linear-phase / FIR families return an error — use
/// [`crate::fir::linear_phase_crossover_taps`] instead.
pub fn crossover_biquad_coefficients(
    family: &str,
    order: usize,
    highpass: bool,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> Result<Vec<[f64; 5]>, String> {
    let xo = Crossover::new(family, order, highpass, freq_hz, sample_rate_hz)?;
    Ok(xo
        .sections
        .iter()
        .map(|s| [s.b0, s.b1, s.b2, s.a1, s.a2])
        .collect())
}

/// True when inverting the higher branch makes the sum flatter around `f_hz`.
pub fn prefer_inverted_high(
    low: &[Complex64],
    high: &[Complex64],
    freqs_hz: &[f64],
    f_hz: f64,
) -> bool {
    let straight = flatness(low, high, freqs_hz, f_hz, false);
    let flipped = flatness(low, high, freqs_hz, f_hz, true);
    flipped + 0.05 < straight
}

fn flatness(
    low: &[Complex64],
    high: &[Complex64],
    freqs_hz: &[f64],
    f_hz: f64,
    invert: bool,
) -> f64 {
    let mut values = Vec::new();
    for ((left, right), freq) in low.iter().zip(high).zip(freqs_hz) {
        if *freq < f_hz / 2.0 || *freq > f_hz * 2.0 {
            continue;
        }
        let sum = if invert {
            *left - *right
        } else {
            *left + *right
        };
        values.push(20.0 * sum.norm().max(1e-20).log10());
    }
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let rms = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    rms.sqrt()
}
