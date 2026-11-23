# seatlock-wasm

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | WebAssembly C-ABI bridge: allocate, free, and JSON `call` for worker methods. |
| **Code** | `engine/crates/seatlock-wasm/` |
| **Spec** | README §3 (build rules) |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Thin wrapper around `seatlock-sim` for the browser. Workers post a method name + JSON; the crate returns JSON (and uses raw buffers for large arrays). No `wasm-bindgen`.

## Responsibilities

- Owns the Wasm export surface used by `web/src/engineWorker.ts` and related workers.
- Does not own UI or real-time audio (`seatlock-rt`).

## How it works

1. Page worker instantiates `seatlock_wasm.wasm`.
2. Calls map to `simulate`, geometry, correction, optimize, listen package, catalog reads, …
3. Design revision is threaded so stale answers can be dropped.

## Checks

- Wasm build in `./check.sh`; browser e2e exercises the worker path.

## See also

- [Simulator](seatlock-sim.md)
- [Web app](Web-app.md)
- [Real-time kernel](seatlock-rt.md)
- [Home](Home.md)
