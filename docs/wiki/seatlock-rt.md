# seatlock-rt

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Real-time audio kernel for the AudioWorklet: same biquad step as DSP, no allocation in `process`. |
| **Code** | `engine/crates/seatlock-rt/` |
| **Spec** | README §3, §10 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Separate Wasm module from the sim worker. The page fetches `.wasm` bytes and instantiates them inside the worklet (no `fetch` there). Up to 16 radiator channels, fixed stage and delay budgets.

## Responsibilities

- Owns sample-accurate playback processing for listen mode.
- Does not own impulse generation (sim) or the React hook (`useListen.ts`).

## How it works

1. Offline sim builds coefficients / convolution data.
2. Worklet loads `seatlock_rt.wasm` and receives numeric parameter buffers only.
3. `seatlock_rt_process` runs fixed-size frames without allocating.

## Checks

- Offline audio / listen tests; browser listen e2e where present.

## See also

- [DSP](seatlock-dsp.md)
- [Web app](Web-app.md)
- [Data flow](Data-flow.md)
- [Home](Home.md)
