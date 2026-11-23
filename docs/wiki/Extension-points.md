# Extension points

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Families registered in one place so new shapes, radiators, radiation models, DSP blocks, and rooms plug in without editing old members. |
| **Code** | `engine/crates/seatlock-core/src/registry.rs`, `seatlock-ext-example` |
| **Spec** | README §3.3; CHK-EXT-1, CHK-EXT-2 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

The lab is built to grow. Each **family** has a contract and a **registry**. Adding a member means new files plus one registration — not edits to every existing member.

## Families

| Family | Contract (short) | Examples |
| --- | --- | --- |
| Outer shape | Mounts, outlines, inside tests, print/stock | `box`, `tube`, `revolved`, `mesh` |
| Internal element | Air/solid role in network and voxels | chamber, duct, brace, voxels |
| Radiator | Z, surface sources per volt, limits | `cone`, `dml_panel` |
| Enclosure model | Cabinet air load on drivers | `lumped`, `waveguide`, `fdtd` |
| Radiation model | Pressure from surface sources | `point`, `baffle`, `exterior`, `bem` |
| Room | In-room addition to direct sound | `free`, `simple` |
| DSP block | Frequency + sample processing | biquad, crossover, delay, … |

## How it works

1. `seatlock-core::Registry` holds parse/create hooks.
2. `seatlock-sim::builtin()` registers the production members.
3. `seatlock-ext-example` proves a third-party crate can register without touching other crates (CHK-EXT-1).
4. Unknown kinds still load and save; the lab warns and skips them (CHK-EXT-2).

## See also

- [Core](seatlock-core.md)
- [Simulator](seatlock-sim.md)
- [Architecture](Architecture.md)
- [Home](Home.md)
