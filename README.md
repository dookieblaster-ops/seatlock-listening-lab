# Seatlock Listening Lab

Native audio and cabinet engine for Seatlock. Version 0.1.0. The phone can open an output stream and play silence generated in Rust. It does not play the lab.

Rust owns the engine. Kotlin owns the Android shell. The browser lab in the parent workspace is the reference the filters have to match. It is not this product. The earlier WebView APK is not this product either.

## Crates

| Crate | Job | Allowed to speak when |
| --- | --- | --- |
| `seatlock-dsp` | Biquads, delay, cone-law waveshaper | A fixed input matches the browser reference sample for sample |
| `seatlock-box` | Sealed and ported model, excursion, port speed | A known alignment hits its published knee |
| `seatlock-geom` | Parametric solid, STL, volume and path length read off the solid | Those litres match the box model |
| `seatlock-catalog` | Drivers with a public source and a date | A sheet has been read into this crate. The list is empty |
| `seatlock-engine` | The callback the phone calls | `fill_silence` writes zeros and counts the frames |
| `seatlock-jni` | The Android bridge into `seatlock-engine` | The `.so` loads and the Kotlin stream calls it |

A fold or a new chamber is not a frequency response. The box formulas do not know about a bend. Until a mesh solver exists, that shape is exported and marked not simulated.

Auto EQ and the other DSP run on this graph later, with the existing rules: no bass boost, no boost from 3 to 5 kHz, and a process turns off when it does not help. They do not get a second audio path.

## Build

```bash
cargo test
```

The engine test checks that silence is written and counted. The other tests still assert that those crates have not claimed a result. A passing suite does not mean the lab sounds right.

The Android app is `com.seatlock.listeninglab`, version 0.1.0. It loads `libseatlock_engine.so`, opens a mono float stream at 48 kHz, and keeps asking Rust for silence. The screen shows the frame count so you can see the callback running.

```bash
export ANDROID_NDK_HOME=/opt/android-sdk/ndk/27.2.12479018
cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build -p seatlock-jni --release
```

Then build the release APK from `android/`. The signing key is not in this repo.

## What is not here

- No filters
- No driver numbers
- No cabinet solid
- No solver for novel geometry

The order of work is in [HANDOFF.md](HANDOFF.md).
