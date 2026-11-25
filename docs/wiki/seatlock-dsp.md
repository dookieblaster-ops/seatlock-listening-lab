# seatlock-dsp

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Digital filter blocks: biquads, crossovers, FIR, delay, gain, polarity, limiter — shared by offline sim and the real-time kernel. |
| **Code** | `engine/crates/seatlock-dsp/` |
| **Spec** | README §7, §10 |
| **Updated** | 2026-11-25 |
| **Status** | living |

## Overview

DSP blocks implement both frequency response (for graphs) and sample processing (for listen / export). Registered into the core registry. No dependency on acoustics crates. `DspBlock::latency_samples` reports FIR/delay group delay (zero for IIR).

## Responsibilities

- Owns filter math, crossover helpers, and FIR kernels.
- Does not own correction planning (`seatlock-correct`) or AudioWorklet glue (`seatlock-rt`).

## Modules and types

| Module | Role |
| --- | --- |
| `lib` | Biquad, gain, delay, polarity, limiter, `fir` registration |
| `crossover` | LR / Butterworth / linear-phase (`family` `linear-phase` or `fir`) |
| `fir` | Explicit-tap FIR; Hann-sinc complementary linear-phase crossover taps |

## How it works

1. Design lists DSP blocks per radiator (and whole-speaker).
2. Sim injects `sample_rate_hz` into each block's fields, evaluates the complex chain response, and notes total FIR/delay latency on the fidelity label.
3. `seatlock-rt` runs the same biquad step on audio samples (FIR stays in the frequency / impulse path for listen).

## Checks

- CHK-DSP-*; `tests/fir.rs` (impulse vs reported H(f); linear-phase sum = delay); listen / passive listen tests.

## See also

- [Correction](seatlock-correct.md)
- [Real-time kernel](seatlock-rt.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
