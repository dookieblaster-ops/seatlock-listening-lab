# Seatlock Listening Lab — software wiki

Living documentation of **how the software is put together**. The product specification stays in [`README.md`](../../README.md). Current build state stays in [`STATUS.md`](../../STATUS.md). Session handoffs stay in [`COMS.md`](../../COMS.md).

This wiki explains **structure and responsibility**: which crate or page owns what, how data moves, and where to look in the code. Keep articles short; link the README for physics formulas and numbered checks.

| Field | Value |
| --- | --- |
| **Kind** | portal |
| **Updated** | 2026-11-23 |
| **Status** | living |
| **How to edit** | [How to maintain](How-to-maintain.md) · [Article template](_ARTICLE_TEMPLATE.md) |

## Quick map

```
Browser page (web/)
  ├─ workers → seatlock-wasm  →  seatlock-sim → acoustics / geom / fdtd / bem / dsp / correct
  └─ AudioWorklet → seatlock-rt → seatlock-dsp
Catalog + designs on disk → loaded into DesignFile (seatlock-core)
```

## Articles by topic

### System

| Article | One-line summary |
| --- | --- |
| [Architecture](Architecture.md) | Layers, dependency direction, what lives where |
| [Data flow](Data-flow.md) | Design → simulate / correct / listen / export |
| [Extension points](Extension-points.md) | Registries for shapes, radiators, radiation, DSP, rooms |
| [Checks and CI](Checks-and-CI.md) | `./check.sh`, CHK-*, GitHub Actions, backups |

### Engine crates (Rust)

| Article | Crate | Role |
| --- | --- | --- |
| [Core](seatlock-core.md) | `seatlock-core` | Design types, catalog load, registry, constants |
| [Geometry](seatlock-geom.md) | `seatlock-geom` | Outer shapes, mounts, mesh import, volumes |
| [Acoustics](seatlock-acoustics.md) | `seatlock-acoustics` | Drivers, boxes, radiation, walls, room, passive |
| [FDTD](seatlock-fdtd.md) | `seatlock-fdtd` | 3D time-domain air grid (inside and outside) |
| [BEM](seatlock-bem.md) | `seatlock-bem` | Exterior boundary-element surface solve |
| [DSP](seatlock-dsp.md) | `seatlock-dsp` | Filters, crossover, FIR, delay, limiter, dynamic bass |
| [Correction](seatlock-correct.md) | `seatlock-correct` | Targets and auto EQ under limits |
| [Simulator](seatlock-sim.md) | `seatlock-sim` | Orchestrates one design into results, score, cost |
| [Wasm bridge](seatlock-wasm.md) | `seatlock-wasm` | JSON C-ABI for Web Workers |
| [Real-time kernel](seatlock-rt.md) | `seatlock-rt` | AudioWorklet processing (no alloc in process) |

### Interface and data

| Article | One-line summary |
| --- | --- |
| [Web app](Web-app.md) | React page, workers, 3D view, listen mode |
| [Catalog and designs](Catalog-and-designs.md) | Sourced parts, examples, imports |

## See also (not wiki)

- Spec: [`README.md`](../../README.md)
- State: [`STATUS.md`](../../STATUS.md)
- Handoffs: [`COMS.md`](../../COMS.md)
- Owner backups: [`../backups.md`](../backups.md)
- New chat paste: [`../new-chat.md`](../new-chat.md)
