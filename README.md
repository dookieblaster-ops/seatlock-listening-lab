# Seatlock Listening Lab

Native audio and cabinet engine for Seatlock. Version 0.0.0. It does not make sound yet.

Rust owns the engine. Kotlin will own the Android app. The browser lab in the parent workspace is the reference the filters have to match. It is not this product. The WebView APK is not this product either.

## Crates

| Crate | Job | Allowed to speak when |
| --- | --- | --- |
| `seatlock-dsp` | Biquads, delay, cone-law waveshaper | A fixed input matches the browser reference sample for sample |
| `seatlock-box` | Sealed and ported model, excursion, port speed | A known alignment hits its published knee |
| `seatlock-geom` | Parametric solid, STL, volume and path length read off the solid | Those litres match the box model |
| `seatlock-catalog` | Drivers with a public source and a date | A sheet has been read into this crate. The list is empty |
| `seatlock-engine` | The callback the phone will call | Step 1 plays silence from Rust. Not before |

A fold or a new chamber is not a frequency response. The box formulas do not know about a bend. Until a mesh solver exists, that shape is exported and marked not simulated.

Auto EQ and the other DSP run on this graph later, with the existing rules: no bass boost, no boost from 3 to 5 kHz, and a process turns off when it does not help. They do not get a second audio path.

## Build

```bash
cargo test
```

Every test currently asserts that the crate has not claimed a result. A passing test means the scaffold builds. It does not mean the lab works.

## What is not here

- No audio callback
- No Android project yet
- No driver numbers
- No solver for novel geometry

The order of work is in [HANDOFF.md](HANDOFF.md).
