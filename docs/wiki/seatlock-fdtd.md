# seatlock-fdtd

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Finite-difference time-domain solver for air inside the cabinet and, as `exterior` radiation, around it. |
| **Code** | `engine/crates/seatlock-fdtd/` |
| **Spec** | README §5.5, §5.6 (`exterior`) |
| **Updated** | 2026-11-25 |
| **Status** | living |

## Overview

Voxel grid of air with a leapfrog update. Used for the `fdtd` enclosure model and for `exterior` radiation (cabinet as a rigid body in an outside block). Supports early stop by harmonic inversion, optional uniform / wall losses, and Rayleigh damping fill (imported damping parts or a chamber's `fill`). Optional field capture feeds the page's 3D wave views.

## Responsibilities

- Owns the 3D grid update, exterior domain, adjoint helpers for inside-shaping, pressure-field capture.
- Does not own everyday baffle radiation or BEM.

## Modules and types

| Module | Role |
| --- | --- |
| `grid` | Voxel occupancy and fields |
| `fields` | Standing-wave / pulse capture for wave views |
| `lib` | Cavity / impedance curve drivers |
| `exterior` | Outside block + absorbing faces |
| `extrapolate` | Early-stop tail fit |
| `adjoint` | Shape-the-inside sensitivities |

## How it works

1. Sim / geom fill solid vs air cells from the cabinet.
2. Time-step until decay criteria or full request.
3. Fourier sums give `Z_ab` and opening transfers `H_k`; exterior reciprocity gives listener transfers.
4. When `Options.capture` is set, the same loop records sparse air-cell standing waves and pulse frames for the page.

## Checks

- CHK-FDTD-1…4; exterior sphere / baffle compare tests.

## See also

- [Geometry](seatlock-geom.md)
- [Acoustics](seatlock-acoustics.md)
- [BEM](seatlock-bem.md)
- [Home](Home.md)
