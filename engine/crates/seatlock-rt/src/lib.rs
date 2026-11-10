//! Real-time test-tone kernel: no allocation inside `seatlock_rt_process`.

#![allow(static_mut_refs, clippy::missing_safety_doc)]

use core::f64::consts::TAU;

/// Fixed output the worklet copies out of. 128 frames is the audio render quantum.
const OUTPUT_FRAMES: usize = 128;

struct RtState {
    sample_rate: f64,
    frequency_hz: f64,
    gain: f32,
    phase: f64,
}

static mut STATE: RtState = RtState {
    sample_rate: 48_000.0,
    frequency_hz: 440.0,
    gain: 0.1,
    phase: 0.0,
};

static mut OUTPUT: [f32; OUTPUT_FRAMES] = [0.0; OUTPUT_FRAMES];

#[no_mangle]
pub extern "C" fn seatlock_rt_init(sample_rate: f64) {
    if sample_rate > 0.0 {
        unsafe {
            STATE.sample_rate = sample_rate;
            STATE.phase = 0.0;
        }
    }
}

#[no_mangle]
pub extern "C" fn seatlock_rt_set_tone(frequency_hz: f64, gain: f64) {
    unsafe {
        STATE.frequency_hz = frequency_hz.max(0.0);
        STATE.gain = gain.clamp(0.0, 1.0) as f32;
    }
}

/// Address of the fixed output buffer inside this module's memory.
#[no_mangle]
pub extern "C" fn seatlock_rt_output_ptr() -> *mut f32 {
    unsafe { OUTPUT.as_mut_ptr() }
}

/// Number of frames `seatlock_rt_output_ptr` can hold.
#[no_mangle]
pub extern "C" fn seatlock_rt_output_capacity() -> usize {
    OUTPUT_FRAMES
}

/// Fill the first `frames` samples of the fixed output buffer. `frames` above the capacity is clamped.
#[no_mangle]
pub unsafe extern "C" fn seatlock_rt_process(frames: usize) {
    let n = frames.min(OUTPUT_FRAMES);
    if n == 0 {
        return;
    }
    let sr = STATE.sample_rate;
    let freq = STATE.frequency_hz;
    let gain = STATE.gain;
    let mut phase = STATE.phase;
    let step = if sr > 0.0 { TAU * freq / sr } else { 0.0 };
    for sample in OUTPUT.iter_mut().take(n) {
        *sample = (phase.sin() as f32) * gain;
        phase += step;
        if phase > TAU {
            phase -= TAU;
        }
    }
    STATE.phase = phase;
}
