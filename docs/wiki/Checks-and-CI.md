# Checks and CI

| Field | Value |
| --- | --- |
| **Kind** | article |
| **Summary** | Local and GitHub suites that keep physics and the page honest; backups are separate owner steps. |
| **Code** | `check.sh`, `tools/cloud-check.sh`, `.github/workflows/check.yml` |
| **Spec** | README §1.3, §13; `docs/backups.md` |
| **Updated** | 2026-11-23 |
| **Status** | living |

## Overview

Trust comes from **named checks** (CHK-*) and a **single command** that runs the whole suite. Agents must not weaken a failing check to make it pass.

## How it works

### Local / agent

- Laptop: `./check.sh` — rustfmt, clippy `-D warnings`, `cargo test --workspace`, wasm build, web typecheck/vitest/build/e2e.
- Cloud agents: `tools/cloud-check.sh` (same steps with cloud workarounds).

### GitHub Actions

- Workflow: `.github/workflows/check.yml`
- Triggers: every PR and every push to `main`
- Runs `./check.sh` with `CI=1` (Playwright installs with OS deps)

Owner must allow Actions once — see [`docs/backups.md`](../backups.md).

### Physics checks

Tables live in README §13 (CHK-DRV, CHK-BOX, CHK-WG, CHK-RAD, CHK-BEM, CHK-WALL, CHK-E2E, …). Each is implemented as Rust tests (and some browser e2e).

## Responsibilities

- **Owns:** how verification is run.
- **Does not own:** the physics content of each CHK (README + crate tests).

## See also

- [Architecture](Architecture.md)
- [Home](Home.md)
- Owner backups: [`../backups.md`](../backups.md)
