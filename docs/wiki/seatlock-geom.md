# seatlock-geom

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Cabinet outer shapes, mounts, imported meshes, volumes, and geometry used by print and voxels. |
| **Code** | `engine/crates/seatlock-geom/` |
| **Spec** | README §6 |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Geometry turns outer-shape fields into volumes, mounting poses, baffle outlines for diffraction, and data the simulator and page need for air and print. Imported 3MF/STL assemblies live here as `mesh`.

## Responsibilities

- Owns outer-shape implementations (`box`, edged shapes, `prism`, `revolved`, `mesh`, …).
- Does not own waveguide/FDTD solves or wall vibration plates (those consume geometry).

## Modules and types

| Module | Role |
| --- | --- |
| `lib` (`BoxShape`, …) | Core box and shared helpers |
| `edged` | Rounded / chamfered / sloped variants |
| `prism` | Extruded outlines |
| `revolved` | Eggs, cylinders, cut spheres |
| `mesh` | Import, hole marking, moulded ports, air columns |

## How it works

1. Registry creates an `OuterShape` from `kind` + fields.
2. Sim asks for interior volume, mounts, outlines, contains tests.
3. Mesh import builds a solid occupancy model; the page marks holes and ports interactively.

## Checks

- CHK-GEO-1, CHK-GEO-2; mesh/import tests; LCM import tests in sim.

## See also

- [Simulator](seatlock-sim.md)
- [FDTD](seatlock-fdtd.md)
- [Web app](Web-app.md)
- [Home](Home.md)
