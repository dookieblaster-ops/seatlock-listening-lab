# Web app

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | React + Vite page: edit designs, show graphs and 3D, drive workers, listen, print. |
| **Code** | `web/src/` |
| **Spec** | README §3, §14 |
| **Updated** | 2026-11-24 |
| **Status** | living |

## Overview

The interface never computes physics. It edits a design document, calls engine workers, and renders results. Print solids use manifold-3d in the page from engine geometry reports.

## Responsibilities

- Owns UI state, workers, 3D view, graphs, import UX, listen hook, OpenSCAD export.
- Does not own formulas inside Rust crates.

## Modules and types

| File / area | Role |
| --- | --- |
| `App.tsx` | Main editor and results layout |
| `useEngine.ts` / `engineWorker.ts` | Sim worker RPC |
| `fdtdWorker.ts` / `jobWorker.ts` | Long jobs with progress |
| `wavefieldWorker.ts` | Pressure-field capture for wave views |
| `CabinetView.tsx` | three.js cabinet view |
| `WaveView.tsx` | Animated inside / outside / room pressure fields |
| `ImportedCabinet.tsx` / `importMesh.ts` | 3MF/STL import and hole marking |
| `Graph.tsx` | Response plots |
| `PassiveEditor.tsx` | Passive R/L/C netlist UI |
| `CostPanel.tsx` / `CostQuality.tsx` / `Generate.tsx` | Cost, optimize, generate |
| `ShapeInside.tsx` | Inside-shaping UX |
| `useListen.ts` | AudioWorklet listen mode |
| `printSolid.ts` / `openscad.ts` | Print and OpenSCAD export |
| `generated/` | Types generated from Rust |

## How it works

1. `main.tsx` mounts the app; `start.sh` serves Vite.
2. On ready, engine worker reports version; catalogs load through Wasm.
3. Edits bump revision; simulate/correct debounce through `useEngine`.
4. Compare overlays (exterior 3D, bem) spawn extra workers so the everyday path stays `baffle`.
5. Wave views call `simulate.wavefield` (inside FDTD, outside BEM probes, room bass grid) and draw in `WaveView`.

## Checks

- Vitest unit tests; Playwright e2e under `web/e2e/`.

## See also

- [Architecture](Architecture.md)
- [Data flow](Data-flow.md)
- [Wasm bridge](seatlock-wasm.md)
- [Home](Home.md)
