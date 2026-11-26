# seatlock-correct

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Auto EQ / correction planner that aims at the target while respecting driver limits. |
| **Code** | `engine/crates/seatlock-correct/` |
| **Spec** | README §8, §11 |
| **Updated** | 2026-11-26 |
| **Status** | living |

## Overview

Builds a list of biquad corrections from simulated (or measured) curves. Skips regions it cannot fix; keeps only filters that help. With `linear_phase` (C7), midband peaking is folded into one linear-phase FIR; bass extension stays IIR. C5 (`resonance`) finds standing-wave peaks vs a coarse FDTD pass. C6 (`dynamic_bass`) plans a look-ahead limiter from free-air cone travel `X/V`. C10 (`indirect`) sets level, band limits, and delay for radiators that do not face the listener when the design has a `simple` room. Metrics helpers compute bass limit, evenness, and clean level used across the lab.

## Responsibilities

- Owns correction feature planning (C1/C2/C5/C6/C7/C10) and shared score metrics helpers.
- Does not own CTA preference rating (that lives in `seatlock-sim` preference module).

## Inputs and outputs

- **In:** frequency grid, SPL, max clean SPL, target bands; optional `linear_phase`; driver Thiele–Small for C6.
- **Out:** `CorrectionOutcome` (IIR filters, optional FIR taps + latency, message) plus C5/C6/C10 blocks applied into the design’s DSP.

## Checks

- CHK-EQ-*; C7 FIR magnitude / fold tests; C5 resonance unit + tall sealed plan test; C6 plan + registry latency; C10 indirect unit + rear-panel plan test; end-to-end correction in CHK-E2E.

## See also

- [DSP](seatlock-dsp.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
