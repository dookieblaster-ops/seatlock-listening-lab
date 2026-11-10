//! Real-time test-tone kernel: no allocation inside `seatlock_rt_process`.

#![allow(static_mut_refs, clippy::missing_safety_doc)]

use core::f64::consts::TAU;

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

#[no_mangle]
pub unsafe extern "C" fn seatlock_rt_process(out_ptr: *mut f32, frames: usize) {
    if out_ptr.is_null() || frames == 0 {
        return;
    }
    let out = std::slice::from_raw_parts_mut(out_ptr, frames);
    let sr = STATE.sample_rate;
    let freq = STATE.frequency_hz;
    let gain = STATE.gain;
    let mut phase = STATE.phase;
    let step = if sr > 0.0 { TAU * freq / sr } else { 0.0 };
    for sample in out.iter_mut() {
        *sample = (phase.sin() as f32) * gain;
        phase += step;
        if phase > TAU {
            phase -= TAU;
        }
    }
    STATE.phase = phase;
}
