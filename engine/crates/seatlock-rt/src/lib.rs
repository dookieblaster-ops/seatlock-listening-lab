//! Real-time kernel. `seatlock_rt_process` does not allocate.
//! It runs the same biquad step as `seatlock-dsp`, plus fixed-budget FIR
//! convolution, and writes one channel per radiator.

#![allow(static_mut_refs, clippy::missing_safety_doc)]

use core::f64::consts::TAU;
use seatlock_dsp::biquad_step;

const FRAMES: usize = 128;
const CHANNELS: usize = 16;
const STAGES: usize = 16;
const DELAY: usize = 512;
/// Max taps per FIR stage (covers 257-tap linear-phase / C7 with headroom).
pub const FIR_LEN: usize = 512;
/// Max FIR stages per channel (e.g. whole-speaker correction + per-radiator XO).
pub const FIR_STAGES: usize = 4;

#[derive(Clone, Copy)]
struct Stage {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Stage {
    const fn wire() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    fn tick(&mut self, x: f64) -> f64 {
        biquad_step(
            self.b0,
            self.b1,
            self.b2,
            self.a1,
            self.a2,
            &mut self.s1,
            &mut self.s2,
            x,
        )
    }
}

#[derive(Clone, Copy)]
struct Fir {
    taps: [f64; FIR_LEN],
    hist: [f64; FIR_LEN],
    n: usize,
    i: usize,
}

impl Fir {
    const fn empty() -> Self {
        Self {
            taps: [0.0; FIR_LEN],
            hist: [0.0; FIR_LEN],
            n: 0,
            i: 0,
        }
    }

    fn from_taps(src: &[f64; FIR_LEN], n: usize) -> Self {
        let n = n.min(FIR_LEN);
        let mut fir = Self::empty();
        fir.n = n;
        fir.taps[..n].copy_from_slice(&src[..n]);
        fir
    }

    fn tick(&mut self, x: f64) -> f64 {
        if self.n == 0 {
            return x;
        }
        if self.n == 1 {
            return self.taps[0] * x;
        }
        self.hist[self.i] = x;
        let mut y = 0.0;
        let mut j = self.i;
        for tap in self.taps.iter().take(self.n) {
            y += tap * self.hist[j];
            j = if j == 0 { self.n - 1 } else { j - 1 };
        }
        self.i += 1;
        if self.i >= self.n {
            self.i = 0;
        }
        y
    }
}

#[derive(Clone, Copy)]
struct Chain {
    stages: [Stage; STAGES],
    count: usize,
    firs: [Fir; FIR_STAGES],
    fir_count: usize,
    gain: f64,
    delay: [f64; DELAY],
    delay_n: usize,
    delay_i: usize,
}

impl Chain {
    const fn unity() -> Self {
        Self {
            stages: [Stage::wire(); STAGES],
            count: 0,
            firs: [Fir::empty(); FIR_STAGES],
            fir_count: 0,
            gain: 1.0,
            delay: [0.0; DELAY],
            delay_n: 0,
            delay_i: 0,
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let mut y = x * self.gain;
        if self.delay_n > 0 {
            let n = self.delay_n.min(DELAY);
            let delayed = self.delay[self.delay_i];
            self.delay[self.delay_i] = y;
            self.delay_i += 1;
            if self.delay_i >= n {
                self.delay_i = 0;
            }
            y = delayed;
        }
        for stage in self.stages.iter_mut().take(self.count) {
            y = stage.tick(y);
        }
        for fir in self.firs.iter_mut().take(self.fir_count) {
            y = fir.tick(y);
        }
        y
    }
}

#[derive(Clone, Copy)]
struct Staged {
    coeffs: [[f64; 5]; STAGES],
    count: usize,
    firs: [[f64; FIR_LEN]; FIR_STAGES],
    fir_lens: [usize; FIR_STAGES],
    fir_count: usize,
    gain: f64,
    delay_n: usize,
}

impl Staged {
    const fn empty() -> Self {
        Self {
            coeffs: [[0.0; 5]; STAGES],
            count: 0,
            firs: [[0.0; FIR_LEN]; FIR_STAGES],
            fir_lens: [0; FIR_STAGES],
            fir_count: 0,
            gain: 1.0,
            delay_n: 0,
        }
    }
}

struct RtState {
    sample_rate: f64,
    source: u32,
    frequency_hz: f64,
    gain: f32,
    phase: f64,
    sweep_hz: f64,
    sweep_ratio: f64,
    pink: [f64; 7],
    noise: u32,
    voltage: f64,
    channels: usize,
    fade: f64,
    fade_step: f64,
    from: [Chain; CHANNELS],
    to: [Chain; CHANNELS],
    staged: [Staged; CHANNELS],
}

const fn chains() -> [Chain; CHANNELS] {
    [Chain::unity(); CHANNELS]
}

static mut STATE: RtState = RtState {
    sample_rate: 48_000.0,
    source: 0,
    frequency_hz: 440.0,
    gain: 0.1,
    phase: 0.0,
    sweep_hz: 20.0,
    sweep_ratio: 1.0,
    pink: [0.0; 7],
    noise: 1,
    voltage: 2.83,
    channels: 1,
    fade: 1.0,
    fade_step: 1.0,
    from: chains(),
    to: chains(),
    staged: [Staged::empty(); CHANNELS],
};

static mut OUTPUT: [[f32; FRAMES]; CHANNELS] = [[0.0; FRAMES]; CHANNELS];
static mut INPUT: [f32; FRAMES] = [0.0; FRAMES];
static mut FIR_SCRATCH: [f64; FIR_LEN] = [0.0; FIR_LEN];

#[no_mangle]
pub extern "C" fn seatlock_rt_init(sample_rate: f64) {
    if sample_rate > 0.0 {
        unsafe {
            STATE.sample_rate = sample_rate;
            STATE.phase = 0.0;
            STATE.fade_step = 1.0 / (0.020 * sample_rate);
        }
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_set_tone(frequency_hz: f64, gain: f64) {
    unsafe {
        STATE.source = 0;
        STATE.frequency_hz = frequency_hz.max(0.0);
        STATE.gain = gain.clamp(0.0, 1.0) as f32;
    }
}

/// `kind` is a number: 0 tone, 1 pink noise, 2 log sweep, 3 samples from the input buffer.
#[no_mangle]
pub extern "C" fn seatlock_rt_set_source(kind: u32, frequency_hz: f64, gain: f64) {
    unsafe {
        STATE.source = kind;
        if kind == 0 {
            STATE.frequency_hz = frequency_hz.max(0.0);
        }
        if kind == 2 {
            STATE.sweep_hz = 20.0;
            let seconds = if frequency_hz > 0.1 {
                frequency_hz
            } else {
                3.0
            };
            let octaves = (20_000.0_f64 / 20.0).ln();
            STATE.sweep_ratio = (octaves / (seconds * STATE.sample_rate)).exp();
        }
        if gain > 0.0 {
            STATE.gain = gain.clamp(0.0, 1.0) as f32;
        }
    }
}

/// Amplifier voltage. 2.83 V leaves the source at the level set by the tone or noise gain.
#[no_mangle]
pub extern "C" fn seatlock_rt_set_voltage(volts: f64) {
    unsafe {
        STATE.voltage = volts.max(0.0);
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_set_channels(count: u32) {
    unsafe {
        STATE.channels = (count as usize).clamp(1, CHANNELS);
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_clear_channel(channel: u32) {
    let index = channel as usize;
    if index >= CHANNELS {
        return;
    }
    unsafe {
        STATE.staged[index] = Staged::empty();
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_set_channel_gain(channel: u32, linear: f64) {
    let index = channel as usize;
    if index >= CHANNELS {
        return;
    }
    unsafe {
        STATE.staged[index].gain = linear;
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_set_delay(channel: u32, samples: u32) {
    let index = channel as usize;
    if index >= CHANNELS {
        return;
    }
    unsafe {
        STATE.staged[index].delay_n = (samples as usize).min(DELAY.saturating_sub(1));
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_add_biquad(
    channel: u32,
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
) -> u32 {
    let index = channel as usize;
    if index >= CHANNELS {
        return 0;
    }
    unsafe {
        let staged = &mut STATE.staged[index];
        if staged.count >= STAGES {
            return staged.count as u32;
        }
        staged.coeffs[staged.count] = [b0, b1, b2, a1, a2];
        staged.count += 1;
        staged.count as u32
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_fir_scratch_ptr() -> *mut f64 {
    unsafe { FIR_SCRATCH.as_mut_ptr() }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_fir_scratch_capacity() -> usize {
    FIR_LEN
}

/// Copy `n` taps from the FIR scratch buffer into the next FIR stage on `channel`.
/// Returns the number of FIR stages on the channel after the add (0 if refused).
#[no_mangle]
pub extern "C" fn seatlock_rt_add_fir(channel: u32, n: u32) -> u32 {
    let index = channel as usize;
    if index >= CHANNELS {
        return 0;
    }
    let n = (n as usize).min(FIR_LEN);
    if n == 0 {
        return unsafe { STATE.staged[index].fir_count as u32 };
    }
    unsafe {
        let staged = &mut STATE.staged[index];
        if staged.fir_count >= FIR_STAGES {
            return staged.fir_count as u32;
        }
        let slot = staged.fir_count;
        staged.firs[slot][..n].copy_from_slice(&FIR_SCRATCH[..n]);
        if n < FIR_LEN {
            staged.firs[slot][n..].fill(0.0);
        }
        staged.fir_lens[slot] = n;
        staged.fir_count += 1;
        staged.fir_count as u32
    }
}

/// Start a 20 ms crossfade from the filters that are playing to the staged ones.
#[no_mangle]
pub extern "C" fn seatlock_rt_commit() {
    unsafe {
        if STATE.fade < 1.0 {
            if STATE.fade >= 0.5 {
                STATE.from = STATE.to;
            }
        } else {
            STATE.from = STATE.to;
        }
        for (chain, staged) in STATE.to.iter_mut().zip(STATE.staged.iter()) {
            *chain = Chain::unity();
            chain.gain = staged.gain;
            chain.delay_n = staged.delay_n;
            chain.count = staged.count.min(STAGES);
            for (stage, coeffs) in chain
                .stages
                .iter_mut()
                .zip(staged.coeffs.iter())
                .take(chain.count)
            {
                *stage = Stage {
                    b0: coeffs[0],
                    b1: coeffs[1],
                    b2: coeffs[2],
                    a1: coeffs[3],
                    a2: coeffs[4],
                    s1: 0.0,
                    s2: 0.0,
                };
            }
            chain.fir_count = staged.fir_count.min(FIR_STAGES);
            for (fir, (taps, &n)) in chain
                .firs
                .iter_mut()
                .zip(staged.firs.iter().zip(staged.fir_lens.iter()))
                .take(chain.fir_count)
            {
                *fir = Fir::from_taps(taps, n);
            }
        }
        STATE.fade = 0.0;
        STATE.fade_step = 1.0 / (0.020 * STATE.sample_rate.max(1.0));
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_output_ptr() -> *mut f32 {
    unsafe { OUTPUT.as_mut_ptr().cast() }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_output_capacity() -> usize {
    FRAMES
}

#[no_mangle]
pub extern "C" fn seatlock_rt_input_ptr() -> *mut f32 {
    unsafe { INPUT.as_mut_ptr() }
}

#[no_mangle]
pub unsafe extern "C" fn seatlock_rt_process(frames: usize) {
    let n = frames.min(FRAMES);
    if n == 0 {
        return;
    }
    let sr = STATE.sample_rate;
    let source = STATE.source;
    let freq = STATE.frequency_hz;
    let gain = f64::from(STATE.gain);
    let scale = STATE.voltage / 2.83;
    let channels = STATE.channels;
    let step = if sr > 0.0 { TAU * freq / sr } else { 0.0 };
    for frame in 0..n {
        let raw = match source {
            1 => pink_sample(),
            2 => sweep_sample(sr),
            3 => f64::from(INPUT[frame]),
            _ => {
                let sample = STATE.phase.sin();
                STATE.phase += step;
                if STATE.phase > TAU {
                    STATE.phase -= TAU;
                }
                sample
            }
        };
        let x = raw * gain * scale;
        let fade = STATE.fade;
        for (channel, slot) in OUTPUT.iter_mut().enumerate().take(channels) {
            let from = STATE.from[channel].run(x);
            let to = STATE.to[channel].run(x);
            let mixed = from * (1.0 - fade) + to * fade;
            slot[frame] = mixed as f32;
        }
        OUTPUT.iter_mut().skip(channels).for_each(|channel| {
            channel[frame] = 0.0;
        });
        if STATE.fade < 1.0 {
            STATE.fade = (STATE.fade + STATE.fade_step).min(1.0);
        }
    }
}

fn pink_sample() -> f64 {
    unsafe {
        STATE.noise = STATE.noise.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (STATE.noise as f64 / u32::MAX as f64) * 2.0 - 1.0;
        let p = &mut STATE.pink;
        p[0] = 0.99886 * p[0] + white * 0.0555179;
        p[1] = 0.99332 * p[1] + white * 0.0750759;
        p[2] = 0.96900 * p[2] + white * 0.1538520;
        p[3] = 0.86650 * p[3] + white * 0.3104856;
        p[4] = 0.55000 * p[4] + white * 0.5329522;
        p[5] = -0.7616 * p[5] - white * 0.0168980;
        let sample = p[0] + p[1] + p[2] + p[3] + p[4] + p[5] + p[6] + white * 0.5362;
        p[6] = white * 0.115926;
        sample * 0.15
    }
}

fn sweep_sample(sample_rate: f64) -> f64 {
    unsafe {
        let sample = STATE.phase.sin();
        STATE.phase += TAU * STATE.sweep_hz / sample_rate.max(1.0);
        if STATE.phase > TAU {
            STATE.phase -= TAU;
        }
        STATE.sweep_hz *= STATE.sweep_ratio;
        if STATE.sweep_hz >= 20_000.0 {
            STATE.sweep_hz = 20.0;
        }
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn a_staged_gain_is_heard_after_the_crossfade() {
        let _guard = LOCK.lock().unwrap();
        unsafe {
            seatlock_rt_init(48_000.0);
            seatlock_rt_set_tone(1_000.0, 0.5);
            seatlock_rt_set_voltage(2.83);
            seatlock_rt_set_channels(1);
            seatlock_rt_clear_channel(0);
            seatlock_rt_set_channel_gain(0, 0.5);
            seatlock_rt_commit();
            for _ in 0..40 {
                seatlock_rt_process(FRAMES);
            }
            seatlock_rt_process(FRAMES);
            let mut energy = 0.0;
            for sample in &OUTPUT[0] {
                energy += f64::from(*sample) * f64::from(*sample);
            }
            let rms = (energy / FRAMES as f64).sqrt();
            let expected = 0.5 * 0.5 / std::f64::consts::SQRT_2;
            assert!(
                (rms - expected).abs() < 0.03,
                "rms {rms} expected {expected}"
            );
        }
    }

    /// Owns the static kernel for the duration of all FIR cases.
    #[test]
    fn fir_convolution_cases() {
        let _guard = LOCK.lock().unwrap();
        unsafe {
            // Unit FIR passes the tone.
            seatlock_rt_init(48_000.0);
            seatlock_rt_set_tone(1_000.0, 0.5);
            seatlock_rt_set_voltage(2.83);
            seatlock_rt_set_channels(1);
            seatlock_rt_clear_channel(0);
            seatlock_rt_set_channel_gain(0, 1.0);
            FIR_SCRATCH[0] = 1.0;
            assert_eq!(seatlock_rt_add_fir(0, 1), 1);
            seatlock_rt_commit();
            for _ in 0..40 {
                seatlock_rt_process(FRAMES);
            }
            seatlock_rt_process(FRAMES);
            let mut energy = 0.0;
            for sample in &OUTPUT[0] {
                energy += f64::from(*sample) * f64::from(*sample);
            }
            let rms = (energy / FRAMES as f64).sqrt();
            let expected = 0.5 / std::f64::consts::SQRT_2;
            assert!(
                (rms - expected).abs() < 0.03,
                "unit fir rms {rms} expected {expected}"
            );

            // Half-gain FIR halves the RMS.
            seatlock_rt_clear_channel(0);
            seatlock_rt_set_channel_gain(0, 1.0);
            FIR_SCRATCH[0] = 0.5;
            assert_eq!(seatlock_rt_add_fir(0, 1), 1);
            seatlock_rt_commit();
            for _ in 0..40 {
                seatlock_rt_process(FRAMES);
            }
            seatlock_rt_process(FRAMES);
            energy = 0.0;
            for sample in &OUTPUT[0] {
                energy += f64::from(*sample) * f64::from(*sample);
            }
            let rms = (energy / FRAMES as f64).sqrt();
            let expected = 0.5 * 0.5 / std::f64::consts::SQRT_2;
            assert!(
                (rms - expected).abs() < 0.03,
                "half fir rms {rms} expected {expected}"
            );

            // taps = [0, 0, 1] → output is input delayed by 2 samples.
            seatlock_rt_set_source(3, 0.0, 1.0);
            seatlock_rt_clear_channel(0);
            seatlock_rt_set_channel_gain(0, 1.0);
            FIR_SCRATCH[0] = 0.0;
            FIR_SCRATCH[1] = 0.0;
            FIR_SCRATCH[2] = 1.0;
            assert_eq!(seatlock_rt_add_fir(0, 3), 1);
            seatlock_rt_commit();
            STATE.fade = 1.0;
            STATE.from = [Chain::unity(); CHANNELS];
            INPUT.fill(0.0);
            INPUT[0] = 1.0;
            seatlock_rt_process(8);
            assert!(
                OUTPUT[0][0].abs() < 1e-6 && OUTPUT[0][1].abs() < 1e-6,
                "early samples {:?}",
                &OUTPUT[0][..3]
            );
            assert!(
                (OUTPUT[0][2] - 1.0).abs() < 1e-5,
                "delayed sample {}",
                OUTPUT[0][2]
            );
            assert!(OUTPUT[0][3].abs() < 1e-6);
        }
    }
}
