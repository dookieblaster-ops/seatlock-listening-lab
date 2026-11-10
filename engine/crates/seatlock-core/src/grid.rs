/// Logarithmic frequency grid. `per_octave` is 48 in the default lab grid.
pub fn log_grid(start_hz: f64, end_hz: f64, per_octave: f64) -> Vec<f64> {
    let octaves = (end_hz / start_hz).log2();
    let steps = (octaves * per_octave).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let f = start_hz * 2_f64.powf(i as f64 / per_octave);
        if f < end_hz {
            out.push(f);
        }
    }
    if out.last().is_none_or(|f| (f - end_hz).abs() > 1e-9) {
        out.push(end_hz);
    }
    out
}

/// README section 4: 10 Hz to 24 kHz, 48 points per octave.
pub fn default_grid() -> Vec<f64> {
    log_grid(10.0, 24_000.0, 48.0)
}
