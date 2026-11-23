# seatlock-core

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Shared constants, design document types, catalog loading, issues, and the kind registry. |
| **Code** | `engine/crates/seatlock-core/` |
| **Spec** | README §3, §4, §9 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Foundation crate. Every other physics crate depends on it. It does not simulate speakers; it defines what a design *is* and how extension families register.

## Responsibilities

- Owns design JSON types, frequency grids, air/sound constants, catalog file loading, warning/error issues, registry hooks.
- Does not own enclosure solves, radiation, or DSP processing.

## Modules and types

| Module | Role |
| --- | --- |
| `design` | `DesignFile` and nested speaker/DSP/build fields |
| `catalog` | Load drivers, materials, stock, fills from `catalog/` |
| `registry` | Traits and registration maps for shapes, radiators, radiation, DSP, rooms |
| `grid` | Log frequency grids |
| `constants` | `C0`, `RHO0`, `P_REF` |
| `issue` | `Issue` / severity for fidelity and user-facing warnings |
| `check` | Lightweight design check + `ENGINE_VERSION` |

## Inputs and outputs

- **In:** paths under `catalog/`, JSON design documents.
- **Out:** typed `DesignFile`, catalog records, a filled `Registry` skeleton for others to register into.

## Checks

- Covered indirectly by almost every engine test; CHK-EXT uses the registry.

## See also

- [Extension points](Extension-points.md)
- [Catalog and designs](Catalog-and-designs.md)
- [Simulator](seatlock-sim.md)
- [Home](Home.md)
