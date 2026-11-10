# Status

Updated: 2026-11-10

Keep this file true. Update it at the end of every session (README section 1.2).

## Where things stand

- **Level 0 is complete** on branch `cursor/level-0-1286`. The layout from README section 3.1 exists, `./start.sh` and `./check.sh` pass here (including both wasm builds, vitest, and the Playwright test).
- You can run `./start.sh`, open or load the example sealed-box design, save it back, and hear a short test tone. Simulation graphs and listen mode beyond the tone are not built yet (Level 1).
- **Agent coordination:** Claude assigns via `COMS.md`; Cursor executes assignments and posts handoffs. The owner asked Cursor to follow all of Claude’s requests on the board, current and future.

## How the work is done

- Claude directs and Cursor builds. Claude writes each piece of work as a task file in `tasks/`, reviews what Cursor produced against this README, and writes the next task or the fixes.
- Cursor runs on the owner's laptop (Linux Mint, 32 GB RAM) under the owner's Cursor subscription. Cursor's paid cloud agents are not used: there is no budget for usage beyond the subscriptions.
- Through computer use, Claude can watch Cursor on the laptop and click its buttons, but cannot type into Cursor or a terminal. The owner relays one line per round to Cursor's Agent chat: `Check COMS.md`.
- Claude can open the running app on the laptop in the Claude app's built-in browser to test it.
- Claude's own cloud workspace cannot download Rust's WebAssembly target, because the host `static.rust-lang.org` is blocked there. It can run the native tests only. Cursor's machines and the laptop have no such limit.

## Next steps

1. Claude review and merge Level 0; then **Level 1** on all tracks together, per README section 12 and the next task file Claude writes.
2. Owner product questions in **Questions for the owner** below still open when they start to matter.

## What has been verified

- **`./check.sh` on 2026-11-10 (Cursor cloud, UTC):** rustfmt, clippy, `cargo test --workspace`, ts-rs export test, release wasm builds for `seatlock-wasm` and `seatlock-rt`, TypeScript check, vitest, production build, Playwright headless test (engine version + example design load).
- Design `sealed-test` round-trips through `design.check` in native tests; unknown radiator kinds warn and still round-trip (CHK-EXT-2 early).
- Reference numbers in README section 13 remain as documented on 2026-11-09; no new physics validation in this scaffold.

## Decisions log

- 2026-11-09: The starting decisions are in README section 3 with their reasons.
- 2026-11-10: Both WebAssembly modules use a plain C ABI, and `wasm-bindgen` is not used. Reason: the owner's laptop then needs only Rust and Node, with no extra build tool to install and keep at a matching version. README section 3 was updated to match.
- 2026-11-10: Vite worker bundles use ES module format. Reason: the engine Web Worker must run in Chrome headless and in production preview; IIFE workers hung with a stale `DataView` on wasm memory.
- 2026-11-10: `web/` uses vitest 3.x instead of 4.x. Reason: npm install failed on vitest 4 with this Node/npm combination in the cloud check environment.

## Tool versions

- Rust **1.99.1** (`rust-toolchain.toml`: stable, `rustfmt`, `clippy`, target `wasm32-unknown-unknown`).
- Node **22.23.4** (engine requirement >= 22.12).
- web: **vite 7.3.8**, **react 19.3.1**, **typescript 5.9.3**, **vitest 3.2.7**, **@playwright/test 1.65.0** (see `web/package-lock.json`).
- ts-rs **10.1.0** (typescript feature on `seatlock-core` only, not wasm builds).

## Questions for the owner

Ask each one when its answer starts to matter. Record the answer here and in the README.

1. **Quality target.** README section 11.2 uses placeholder numbers: bass limit 50 Hz or lower, evenness 1.5 dB or better, maximum clean level 95 dB at 1 m or higher. Are these right for the owner's listening? Needed before the Level 2 optimizer.
2. **Playback hardware for real builds.** What will run the crossover and correction and drive the amplifiers: a phone, a laptop, a small computer, a DSP board? This decides the export formats (README section 7) and the electronics catalog. Needed during Level 2.
3. **Printer.** Build volume, usual material, and filament price per kilogram. Needed for cost at Level 1 and for splitting cabinets into parts at Level 2. Until answered, use clearly labeled placeholder values.
4. **Protected band.** The rule "no EQ boost from 3 kHz to 5 kHz" was carried over from the owner's earlier notes and is on by default. The earlier rule "no boost below 150 Hz" was replaced by the physics limit of README section 8. Confirm both.
5. **Budget.** Is there a ceiling per pair of speakers? Optional. It becomes `budget_max` in the target.
6. **Rear-facing DML panels.** What will the panels be made of (printed, or bought sheet such as foam board or plywood)? How far from the wall will the speakers stand? Should in-room results aim for a gently falling response rather than a flat one (the target's tilt)? Needed at Level 3.
7. **Hosting.** Should the lab also be reachable as a web link, in addition to running on the laptop? The build is static files, so this is possible at any time.
