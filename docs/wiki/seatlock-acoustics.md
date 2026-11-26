# seatlock-acoustics

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Drivers, enclosure loads, radiation models, wall vibration, room estimate, and passive networks. |
| **Code** | `engine/crates/seatlock-acoustics/` |
| **Spec** | README §5 (esp. 5.1–5.6, 5.9–5.11), §7 |
| **Updated** | 2026-11-26 |
| **Status** | living |

## Overview

Core acoustic models for everyday simulation: cone (and DML) radiators, sealed/vented/waveguide loads (tree or looped mesh), passive-radiator Map/Cap/Rap branches, chamber damping fill (Delany–Bazley Cap), point and baffle radiation, simple room, vibrating walls, and passive R/L/C nets.

## Responsibilities

- Owns radiator and load physics used by `seatlock-sim`.
- Does not own FDTD grids or BEM surface meshes (separate crates); sim joins them.

## Modules and types

| Module | Role |
| --- | --- |
| `cone` | Thiele–Small terminal and mechanics |
| `sealed` / `vented` | Quick enclosure loads |
| `waveguide` | Detailed duct/chamber network (tree or nodal mesh for loops); radiation matrix join |
| `network` | Multi-chamber lumped wiring (ports, optional PR branches, optional fill Cap) |
| `fill` | Delany–Bazley / Allard–Champoux complex bulk modulus from flow resistivity; `chamber_admittance_with_parts` for parallel Caps (imported damping volumes) |
| `passive_radiator` | Small Map/Cap/Rap branch; `sealed_with_pr` |
| `radiation` / `baffle` / `point` | Everyday radiation |
| `room` | `simple` room estimate |
| `wall` / `laminate` / `plate` | Wall and panel vibration |
| `dml` | DML panel radiator |
| `passive` | Passive R/L/C solve with driver Z |
| `response` | Blend maker measured curves above piston band |

## How it works

Sim builds radiator terminals, enclosure load `Z_ab` / network, applies walls as shunts when requested, then radiates surface sources with the chosen radiation model and optional room.

## Checks

- CHK-DRV-*, CHK-BOX-*, CHK-WG-*, CHK-RAD-1/2, CHK-ROOM-1, CHK-WALL-*, CHK-PLATE-*, passive tests.

## See also

- [FDTD](seatlock-fdtd.md)
- [BEM](seatlock-bem.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
