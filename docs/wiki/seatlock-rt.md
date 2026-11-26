# seatlock-rt

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Real-time audio kernel for the AudioWorklet: same biquad step as DSP plus fixed-budget FIR convolution; no allocation in `process`. |
| **Code** | `engine/crates/seatlock-rt/` |
| **Spec** | README §3, §7, §10 |
| **Updated** | 2026-11-26 |
| **Status** | living |

## Overview

Separate Wasm module from the sim worker. The page fetches `.wasm` bytes and instantiates them inside the worklet (no `fetch` there). Up to 16 radiator channels, fixed biquad stage, delay, and FIR budgets (`FIR_STAGES` × `FIR_LEN` = 4 × 512).

## Responsibilities

- Owns sample-accurate playback processing for listen mode (biquads + FIR).
- Does not own impulse generation (sim) or the React hook (`useListen.ts`).

## How it works

1. Offline sim builds coefficients / FIR taps / convolution data (`listen_package`).
2. Worklet loads `seatlock_rt.wasm` and receives numeric parameter buffers only; FIR taps are copied into a scratch buffer then `seatlock_rt_add_fir`.
3. `seatlock_rt_process` runs fixed-size frames without allocating (gain → delay → biquads → FIR cascade).

## Checks

- Native `seatlock-rt` tests (gain, unit FIR, half-gain FIR, delay-line impulse); browser listen / tone e2e where present.

## See also

- [DSP](seatlock-dsp.md)
- [Web app](Web-app.md)
- [Data flow](Data-flow.md)
- [Home](Home.md)
