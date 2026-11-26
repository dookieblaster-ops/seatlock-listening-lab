# seatlock-sim

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Orchestrates a design into simulation results, cost, preference score, optimizer, and generator. |
| **Code** | `engine/crates/seatlock-sim/` |
| **Spec** | README §3.2, §9, §11 |
| **Updated** | 2026-11-25 |
| **Status** | living |

## Overview

Front door of the engine after Wasm unwraps JSON. `simulate` wires geometry, acoustics, optional FDTD/BEM, DSP, walls, and fidelity labels. Sibling modules handle cost, optimize, generate, preference (CTA-2034 / Olive), compare, and import helpers.

## Responsibilities

- Owns end-to-end simulation orchestration and product-facing reports (cost, preference, generate).
- Does not own low-level CBIE or FDTD kernels.

## Modules and types

| Module | Role |
| --- | --- |
| `lib` | `simulate`, `builtin` registry, listen package |
| `bem` | Wire `bem` radiation + mouth Z into waveguide |
| `walls` | Build vibrating wall stacks from catalog materials |
| `chambers` / `ports` | Multi-chamber, chamber `fill` (damping), moulded openings, passive-radiator ducts, duct-mounted drivers |
| `preference` | CTA-2034 / Olive preference |
| `cost` | Line-item build cost |
| `optimize` / `generate` | Search and cabinet proposals |
| `exterior` / `shape` / `voxels` | 3D exterior and inside-shaping hooks |
| `wavefield` | Inside / outside / room pressure fields for the page |
| `compare` | Kit copies vs published curves |

## How it works

1. `builtin()` registers all production members.
2. Parse design + request → evaluate each radiator through enclosure + radiation + room.
3. Attach fidelity “left out” lines; compute scores/cost when asked.

## Checks

- CHK-E2E-*; crate integration tests (`bem`, `walls`, `wavefield`, `criton`, `lcm`, `generate`, …).

## See also

- [Data flow](Data-flow.md)
- [Acoustics](seatlock-acoustics.md)
- [BEM](seatlock-bem.md)
- [Wasm bridge](seatlock-wasm.md)
- [Home](Home.md)
