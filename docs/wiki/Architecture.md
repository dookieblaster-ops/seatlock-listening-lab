# Architecture

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Layered browser app: React UI, Web Workers with Rust/Wasm physics, AudioWorklet with a separate real-time kernel. |
| **Code** | `engine/`, `web/`, `catalog/`, `designs/` |
| **Spec** | README §3 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Seatlock Listening Lab is a **local, offline** speaker design simulator. All physics, scoring, and DSP live in Rust. The browser only edits designs and shows results. Two Wasm modules are built: one for workers (`seatlock-wasm`) and one for the AudioWorklet (`seatlock-rt`).

## Responsibilities

- **Owns:** overall layering and dependency direction.
- **Does not own:** individual physics models (see crate articles).

## How it works

1. The page holds a JSON design document (same schema as files under `designs/`).
2. Simulation requests go to a Web Worker that loads `seatlock_wasm.wasm` and calls JSON methods (`simulate`, `correct`, `optimize`, …).
3. Long jobs (full 3D, generate, exterior overlay) use dedicated workers with progress and cancel.
4. Listen mode posts coefficients and impulses into an AudioWorklet running `seatlock_rt.wasm`.
5. Print solids for STL/3MF are built in the page (`web/src/printSolid.ts`) from geometry reports the engine already computed — the one intentional physics-adjacent exception called out in the README.

## Dependency direction (engine)

```
seatlock-core
    ↑
seatlock-geom, seatlock-dsp
    ↑
seatlock-acoustics, seatlock-fdtd, seatlock-bem, seatlock-correct
    ↑
seatlock-sim
    ↑
seatlock-wasm

seatlock-dsp → seatlock-rt   (real-time path; no physics crates)
```

Nothing under `engine/` depends on the browser except the two wrapper crates. DSP never depends on acoustics.

## Layers

| Layer | Location | Job |
| --- | --- | --- |
| Spec / process | `README.md`, `STATUS.md`, `COMS.md` | What to build and what is true |
| Catalog | `catalog/` | Sourced drivers, materials, stock |
| Designs | `designs/` | Examples and imported kit copies |
| Engine | `engine/crates/*` | Physics and orchestration |
| Page | `web/src/*` | Edit, display, print, listen |
| Checks | `./check.sh`, `.github/workflows/` | Keep the suite green |

## See also

- [Data flow](Data-flow.md)
- [Extension points](Extension-points.md)
- [Web app](Web-app.md)
- [Home](Home.md)
