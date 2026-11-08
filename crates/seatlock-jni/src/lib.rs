//! JNI entry the Android app calls. The samples come from `seatlock-engine`.

use jni::objects::{JClass, JFloatArray};
use jni::sys::{jint, jlong};
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_com_seatlock_listeninglab_Engine_fillSilence(
    env: JNIEnv,
    _class: JClass,
    output: JFloatArray,
) -> jint {
    let len = env.get_array_length(&output).unwrap_or(0);
    if len <= 0 {
        return 0;
    }
    let mut buf = vec![0.0_f32; len as usize];
    let written = seatlock_engine::fill_silence(&mut buf) as jint;
    if env.set_float_array_region(&output, 0, &buf).is_err() {
        return 0;
    }
    written
}

#[no_mangle]
pub extern "system" fn Java_com_seatlock_listeninglab_Engine_framesFilled(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    seatlock_engine::frames_filled() as jlong
}
