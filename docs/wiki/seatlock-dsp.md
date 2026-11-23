# seatlock-dsp

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Digital filter blocks: biquads, crossovers, delay, gain, polarity, limiter — shared by offline sim and the real-time kernel. |
| **Code** | `engine/crates/seatlock-dsp/` |
| **Spec** | README §7, §10 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

DSP blocks implement both frequency response (for graphs) and sample processing (for listen / export). Registered into the core registry. No dependency on acoustics crates.

## Responsibilities

- Owns filter math and crossover helpers.
- Does not own correction planning (`seatlock-correct`) or AudioWorklet glue (`seatlock-rt`).

## Modules and types

| Module | Role |
| --- | --- |
| `lib` | Biquad, gain, delay, polarity, limiter registration |
| `crossover` | LR / Butterworth split helpers |

## How it works

1. Design lists DSP blocks per radiator (and whole-speaker).
2. Sim evaluates complex response along the chain.
3. `seatlock-rt` runs the same biquad step on audio samples.

## Checks

- CHK-DSP-*; listen / passive listen tests.

## See also

- [Correction](seatlock-correct.md)
- [Real-time kernel](seatlock-rt.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
