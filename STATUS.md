# Status

Updated: 2026-11-10

Keep this file true. Update it at the end of every session (README section 1.2).

## Where things stand

- **Level 0 has not started.** The `main` branch holds the specification only: `README.md`, this file, `COMS.md`, and the agent pointer files. There is no program yet.
- **Agent coordination:** Claude (lead architect) and the Cursor helper use `COMS.md` per README section 1.4. Assignment 1 (Level 0, `tasks/level-0.md`) posted there on 2026-11-10. Cursor works on its own branch; Claude reviews and merges into `main`.
- The specification was rewritten on 2026-11-09, when the owner redefined the product as a speaker design simulator (README section 2).
- Later on 2026-11-09 the owner widened it: the cabinet's outside may be any novel shape, and the lab must take novel radiators such as rear-facing DML panels. The README gained extension points (3.3), open outer shapes and mounts (6.2), radiators as a family with surface sources (5.6), DML panels (5.9), a simple room estimate (5.10), correction feature C10, and the checks CHK-EXT-1, CHK-EXT-2, CHK-RAD-1, CHK-PLATE-1, CHK-ROOM-1.
- The earlier work is kept on the `legacy` branch: a native Android shell that played silence from a Rust engine. It is not this product. Do not port from it.
- An earlier browser prototype existed outside this repository. It is not a reference for this build.

## How the work is done

- Claude directs and Cursor builds. Claude writes each piece of work as a task file in `tasks/`, reviews what Cursor produced against this README, and writes the next task or the fixes.
- Cursor runs on the owner's laptop (Linux Mint, 32 GB RAM) under the owner's Cursor subscription. Cursor's paid cloud agents are not used: there is no budget for usage beyond the subscriptions.
- Through computer use, Claude can watch Cursor on the laptop and click its buttons, but cannot type into Cursor or a terminal. The owner relays one line per round to Cursor's Agent chat: `Check COMS.md`.
- Claude can open the running app on the laptop in the Claude app's built-in browser to test it.
- Claude's own cloud workspace cannot download Rust's WebAssembly target, because the host `static.rust-lang.org` is blocked there. It can run the native tests only. Cursor's machines and the laptop have no such limit.

## Next steps

1. Level 0, written up as a ready-to-run task in `tasks/level-0.md`. Level 0 from README section 12: the repository layout, `start.sh`, `check.sh`, the engine answering a call from a worker, the worklet test tone, and design file round-trip.
2. Then Level 1, all tracks together, following the level rule.

## What has been verified

- The reference numbers in README section 13 were computed on 2026-11-09 with a throwaway Python implementation of the README's equations. It is not in the repository. It covered the driver model, sealed and vented `lumped` boxes, the enclosure load interface, a `waveguide` port and resonator, biquad and crossover sums, and a small `fdtd` solver with a resistive source, semi-implicit boundaries, and an absorbing exterior block.
- That solver reproduced box resonances within 0.2 % and a Helmholtz resonance within 0.2 % of the `lumped` value. An explicit boundary update went unstable, which is where the rule in README section 5.5 comes from.
- CHK-FDTD-4 and the time budgets in section 5.5 are estimates. Nothing has measured them yet.
- CHK-PLATE-1 is the closed-form result, cross-checked with a finite-difference eigenvalue solve (within 0.1 %). CHK-RAD-1 and CHK-ROOM-1 were computed from their formulas.
- Not validated by anything yet: the exciter and backing equations of section 5.9, the `exterior` radiation model and its reciprocity run (5.6), the `baffle` model's edge diffraction, and the `simple` room's reflection term. Treat them as starting designs.
- Three tool facts were checked against current documentation on 2026-11-09: the status of `wasm-pack` and `wasm-bindgen`, how `manifold-3d` is loaded and released, and how Claude Code loads `AGENTS.md` and its imports. The other tool choices are long established and were not rechecked. Nothing has been built with any of them yet.

## Decisions log

One line per decision: date, decision, reason. Decisions that define the product live in the README, not here.

- 2026-11-09: The starting decisions are in README section 3 with their reasons.
- 2026-11-10: Both WebAssembly modules use a plain C ABI, and `wasm-bindgen` is not used. Reason: the owner's laptop then needs only Rust and Node, with no extra build tool to install and keep at a matching version. README section 3 was updated to match.

## Tool versions

Record pinned versions here at Level 0.

## Questions for the owner

Ask each one when its answer starts to matter. Record the answer here and in the README.

1. **Quality target.** README section 11.2 uses placeholder numbers: bass limit 50 Hz or lower, evenness 1.5 dB or better, maximum clean level 95 dB at 1 m or higher. Are these right for the owner's listening? Needed before the Level 2 optimizer.
2. **Playback hardware for real builds.** What will run the crossover and correction and drive the amplifiers: a phone, a laptop, a small computer, a DSP board? This decides the export formats (README section 7) and the electronics catalog. Needed during Level 2.
3. **Printer.** Build volume, usual material, and filament price per kilogram. Needed for cost at Level 1 and for splitting cabinets into parts at Level 2. Until answered, use clearly labeled placeholder values.
4. **Protected band.** The rule "no EQ boost from 3 kHz to 5 kHz" was carried over from the owner's earlier notes and is on by default. The earlier rule "no boost below 150 Hz" was replaced by the physics limit of README section 8. Confirm both.
5. **Budget.** Is there a ceiling per pair of speakers? Optional. It becomes `budget_max` in the target.
6. **Rear-facing DML panels.** What will the panels be made of (printed, or bought sheet such as foam board or plywood)? How far from the wall will the speakers stand? Should in-room results aim for a gently falling response rather than a flat one (the target's tilt)? Needed at Level 3.
7. **Hosting.** Should the lab also be reachable as a web link, in addition to running on the laptop? The build is static files, so this is possible at any time.
