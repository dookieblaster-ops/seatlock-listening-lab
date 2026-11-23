# seatlock-bem

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Exterior collocation boundary-element solve on a triangulated outer skin; optional radiation member `bem`. |
| **Code** | `engine/crates/seatlock-bem/`, `engine/crates/seatlock-sim/src/bem.rs` |
| **Spec** | README §2.6, §5.6 (`bem`); CHK-BEM-1…10 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Frequency-domain Helmholtz CBIE on constant triangles (CHIEF for irregular frequencies). Meshes the true outer shape from an AABB seed (star-convex pull-in) or an occupancy remesh for re-entrant shells. Mouths are true open apertures. Mouth `Z_rad` matrices join the waveguide network.

## Responsibilities

- Owns surface mesh construction, CBIE / CHIEF solve, field representation, mouth impedances.
- Experimental: Burton–Miller, discontinuous linear, spherical-triangle variants (not radiation defaults).
- Does not own the page overlay worker (web) or waveguide iteration (sim/acoustics).

## Modules and types

| Module | Role |
| --- | --- |
| `mesh` / `occupancy` | AABB seed, open apertures, re-entrant remesh |
| `lib` | Constant-element CBIE + CHIEF |
| `linear` / `curved` / `continuous` / `burton_miller` | Higher-order / experimental systems |

## How it works

1. Build closed triangle mesh of the outer cabinet (+ open mouth disks).
2. One LU (or normal equations with CHIEF) per frequency.
3. Drive mouths / patches; read surface pressure; representation formula at listeners.
4. Sim hands over to baffle above the mesh frequency limit; sound power still baffle.

## Checks

- CHK-BEM-1…10; sim `bem` integration tests.

## See also

- [Acoustics](seatlock-acoustics.md)
- [Simulator](seatlock-sim.md)
- [FDTD](seatlock-fdtd.md) (alternative exterior path)
- [Home](Home.md)
