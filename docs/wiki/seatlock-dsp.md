# seatlock-dsp

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Digital filter blocks: biquads, crossovers, FIR, delay, gain, polarity, limiter — shared by offline sim and the real-time kernel. |
| **Code** | `engine/crates/seatlock-dsp/` |
| **Spec** | README §7, §10 |
| **Updated** | 2026-11-26 |
| **Status** | living |

## Overview

DSP blocks implement both frequency response (for graphs) and sample processing (for listen / export). Registered into the core registry. No dependency on acoustics crates. `DspBlock::latency_samples` reports FIR/delay group delay (zero for IIR). Mono IEEE float WAV helpers export FIR impulses for hardware.

## Responsibilities

- Owns filter math, crossover helpers, FIR kernels, and WAV packing for FIR export.
- Does not own correction planning (`seatlock-correct`) or AudioWorklet glue (`seatlock-rt`).

## Modules and types

| Module | Role |
| --- | --- |
| `lib` | Biquad, gain, delay, polarity, limiter, `fir` registration |
| `crossover` | LR / Butterworth / linear-phase (`family` `linear-phase` or `fir`); IIR coeff export |
| `fir` | Explicit-tap FIR; Hann-sinc complementary linear-phase crossover taps |
| `wav` | Mono IEEE float32 WAV encode/decode for FIR export |

## How it works

1. Design lists DSP blocks per radiator (and whole-speaker).
2. Sim injects `sample_rate_hz` into each block's fields, evaluates the complex chain response, and notes total FIR/delay latency on the fidelity label.
3. `listen_package` expands IIR crossovers to biquad stages for Play/`filters.txt`, and packs FIR / linear-phase taps as mono WAVs.
4. `seatlock-rt` runs the same biquad step and fixed-budget FIR convolution on audio samples.

## Checks

- CHK-DSP-*; `tests/fir.rs` (impulse vs reported H(f); linear-phase sum = delay); `wav` round-trip; listen FIR WAV + IIR crossover expand tests.

## See also

- [Correction](seatlock-correct.md)
- [Real-time kernel](seatlock-rt.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
