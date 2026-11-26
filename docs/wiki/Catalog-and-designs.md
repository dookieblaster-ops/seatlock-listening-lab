# Catalog and designs

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | On-disk sourced parts and design documents the engine and page load; no invented numbers. |
| **Code** | `catalog/`, `designs/`, `targets/` |
| **Spec** | README §9, §1.3 |
| **Updated** | 2026-11-25 |
| **Status** | living |

## Overview

Drivers, materials, stock, fills, and electronics live as JSON with source URLs and `read_on` dates. Example designs and imported kits live under `designs/`. Quality targets live under `targets/`.

## Responsibilities

- Owns purchasable/sourced data and shipped designs.
- Does not own simulation math (records are inputs to the engine).

## Layout

| Path | Contents |
| --- | --- |
| `catalog/drivers/` | Driver T/S, prices, optional measured curves |
| `catalog/materials.json` | Print filaments (incl. foaming) |
| `catalog/panels/`, `fills/`, `damping/`, `stock/` | Panel, wall fill, damping (also chamber `fill`), sheet/pipe stock |
| `catalog/electronics/`, `exciters/`, `printers/` | Amps, DSP, exciters (incl. sourced Dayton DAEX25FHE-4 / DAEX25VT-4), printer rates |
| `catalog/passive-radiators/` | Passive radiators (Dayton DSA175-PR) |
| `designs/examples/` | Lab examples (`sealed-test`, `folded-line`, `pr-box`, …) |
| `designs/imports/` | Kit copies (LCM, C-Note, Criton, …) with licenses |
| `targets/` | e.g. `hifi-default.json` with `preference_min` |

## Rules

- Do not invent parameters; open a source in-session (README §1.3).
- Missing published loss factors use labeled lab assumptions (see STATUS / materials).

## See also

- [Core](seatlock-core.md)
- [Acoustics](seatlock-acoustics.md) (consumes materials for walls)
- [Home](Home.md)
