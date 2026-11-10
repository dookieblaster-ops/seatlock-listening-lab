# Status

Updated: 2026-11-09

Keep this file true. Update it at the end of every session (README section 1.2).

## Where things stand

- **Level 0 has not started.** The `main` branch holds the specification only: `README.md`, this file, and the agent pointer files. There is no program yet.
- The specification was rewritten on 2026-11-09, when the owner redefined the product as a speaker design simulator (README section 2).
- The earlier work is kept on the `legacy` branch: a native Android shell that played silence from a Rust engine. It is not this product. Do not port from it.
- An earlier browser prototype existed outside this repository. It is not a reference for this build.

## Next steps

1. Level 0 from README section 12: the repository layout, `start.sh`, `check.sh`, the engine answering a call from a worker, the worklet test tone, and design file round-trip.
2. Then Level 1, all tracks together, following the level rule.

## What has been verified

- The reference numbers in README section 13 were computed on 2026-11-09 with a throwaway Python implementation of the README's equations. It is not in the repository. It covered the driver model, sealed and vented `lumped` boxes, the enclosure load interface, a `waveguide` port and resonator, biquad and crossover sums, and a small `fdtd` solver with a resistive source, semi-implicit boundaries, and an absorbing exterior block.
- That solver reproduced box resonances within 0.2 % and a Helmholtz resonance within 0.2 % of the `lumped` value. An explicit boundary update went unstable, which is where the rule in README section 5.5 comes from.
- CHK-FDTD-4 and the time budgets in section 5.5 are estimates. Nothing has measured them yet.
- Three tool facts were checked against current documentation on 2026-11-09: the status of `wasm-pack` and `wasm-bindgen`, how `manifold-3d` is loaded and released, and how Claude Code loads `AGENTS.md` and its imports. The other tool choices are long established and were not rechecked. Nothing has been built with any of them yet.

## Decisions log

One line per decision: date, decision, reason. Decisions that define the product live in the README, not here.

- 2026-11-09: No entries yet. The starting decisions are in README section 3 with their reasons.

## Tool versions

Record pinned versions here at Level 0.

## Questions for the owner

Ask each one when its answer starts to matter. Record the answer here and in the README.

1. **Quality target.** README section 11.2 uses placeholder numbers: bass limit 50 Hz or lower, evenness 1.5 dB or better, maximum clean level 95 dB at 1 m or higher. Are these right for the owner's listening? Needed before the Level 2 optimizer.
2. **Playback hardware for real builds.** What will run the crossover and correction and drive the amplifiers: a phone, a laptop, a small computer, a DSP board? This decides the export formats (README section 7) and the electronics catalog. Needed during Level 2.
3. **Printer.** Build volume, usual material, and filament price per kilogram. Needed for cost at Level 1 and for splitting cabinets into parts at Level 2. Until answered, use clearly labeled placeholder values.
4. **Protected band.** The rule "no EQ boost from 3 kHz to 5 kHz" was carried over from the owner's earlier notes and is on by default. The earlier rule "no boost below 150 Hz" was replaced by the physics limit of README section 8. Confirm both.
5. **Budget.** Is there a ceiling per pair of speakers? Optional. It becomes `budget_max` in the target.
6. **Hosting.** Should the lab also be reachable as a web link, in addition to running on the laptop? The build is static files, so this is possible at any time.
