# Data flow

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | One design document passes through simulate, correct, score, cost, listen, and export. |
| **Code** | `seatlock-sim`, `seatlock-wasm`, `web/src/useEngine.ts`, `web/src/useListen.ts` |
| **Spec** | README §2.1, §3.2 |
| **Updated** | 2026-11-24 |
| **Status** | living |

## Overview

Everything the lab shows comes from **one design** through **one chain**. The interface never recomputes physics; it asks the engine and draws the answer.

## How it works

### Simulate

```
DesignFile + SimulateRequest
  → seatlock_sim::simulate*
  → per-radiator and summed curves, impedance, limits, fidelity label, score hooks, cost hooks
```

Request fields choose enclosure model (`lumped` / `waveguide` / `fdtd`), radiation (`point` / `baffle` / `exterior` / `bem`), room, listening window, CTA preference, losses, and walls.

### Correct

```
Design + measured/sim curves
  → seatlock_correct
  → biquad plan written back into the design’s DSP
```

Correction respects driver limits and skips bands it cannot fix (README §8).

### Score and optimize

```
simulate (often with cta2034) → preference / shortfall → optimizer or generator candidates
```

Preference rating and `preference_min` are the ranked quality target (STATUS / README §11).

### Listen

```
simulate → impulse / coefficient package
  → AudioWorklet + seatlock-rt
  → one channel per radiator (up to 16)
```

Passive boards fold into listen impulses; leftover digital radiator blocks are not played when `dsp.passive` is set.

### Wave views

```
Design + WaveFieldRequest (inside | outside | room)
  → seatlock_sim::wavefield → CapturedField
  → wavefieldWorker → WaveView (standing wave / pulse)
```

Inside uses FDTD field capture; outside samples BEM exterior pressure on a probe grid; room is a coarse bass FDTD of an assumed rectangular room from the simple-room volume.

### Export

- STL / 3MF / OpenSCAD from print solids (page + geometry report).
- Filter coefficients for external DSP boards.

## Revision rule

Each engine request carries a **design revision**. Stale results for an older revision are discarded so the UI never shows answers for a design the user already changed.

## See also

- [Architecture](Architecture.md)
- [Simulator](seatlock-sim.md)
- [Wasm bridge](seatlock-wasm.md)
- [Real-time kernel](seatlock-rt.md)
- [Home](Home.md)
