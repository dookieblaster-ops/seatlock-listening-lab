# seatlock-correct

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Auto EQ / correction planner that aims at the target while respecting driver limits. |
| **Code** | `engine/crates/seatlock-correct/` |
| **Spec** | README §8, §11 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Builds a list of biquad corrections from simulated (or measured) curves. Skips regions it cannot fix; keeps only filters that help. Metrics helpers compute bass limit, evenness, and clean level used across the lab.

## Responsibilities

- Owns correction feature planning and shared score metrics helpers.
- Does not own CTA preference rating (that lives in `seatlock-sim` preference module).

## Inputs and outputs

- **In:** frequency grid, SPL, max clean SPL, target bands.
- **Out:** `CorrectionOutcome` (filters + message) applied into the design’s DSP.

## Checks

- CHK-EQ-*; end-to-end correction in CHK-E2E.

## See also

- [DSP](seatlock-dsp.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
