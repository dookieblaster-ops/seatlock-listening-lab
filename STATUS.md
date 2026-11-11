# Status

Updated: 2026-11-10

Keep this file true. Update it at the end of every session (README section 1.2).

## Where things stand

- **Level 0 is on `main`, including Assignment 2, merged by Claude on 2026-11-10.** The layout, worker, design load and save, and scripts exist. The test tone now comes from a fixed buffer inside the real-time kernel, and an offline audio check covers it.
- **Level 1 milestone M1** is merged into `main` (Assignment 3, merged by Claude on 2026-11-10). One sealed driver can be simulated and graphed.
- **Level 1 milestone M2** is merged into `main` (Assignment 4, merged by Claude on 2026-11-10). The cabinet can be viewed and exported for printing, and cost says the price is unknown until prices are filled in.
- **Level 1 milestone M3**, with the M2 browser fixes, is merged into `main` (Assignment 5, merged by Claude on 2026-11-10). The 3D air model runs in the browser, correction was reworked in Assignment 6, and bass limit, evenness, and maximum clean level are on screen. Full listening is still later.
- **Assignment 6** (correction rework and Level 1 M4) is merged into `main` (merged by Claude on 2026-11-10). Level 1 is complete except three sourced real drivers (Assignment 7) and the laptop run. Correction skips regions it cannot fix, aims at the bass-shaped target, and keeps the filters it actually uses inside the driver's limits. Listen mode plays pink noise, a log sweep, a tone, or a file through the real-time kernel and one convolver per radiator.
- **Assignment 7** (three sourced drivers and Level 2 M1) is merged into `main` (merged by Claude on 2026-11-10). Level 1 is complete apart from the laptop run. The catalog has three Dayton Audio drivers copied from sheets opened on 2026-11-10. A vented two-way can be simulated, graphed per driver, and compared with a second design.
- **Assignment 8** (Level 2 M2) is merged into `main` (merged by Claude on 2026-11-10). Waveguide checks CHK-WG-1, CHK-WG-2, and CHK-WG-3 pass. Baffle checks CHK-RAD-1 and CHK-RAD-2 pass, and a `baffle` request is about 6 dB below half-space at low frequency. The catalog has twelve sourced drivers (nine budget, three reference), two budget amplifier boards, one reference amplifier, one DSP option, and five stock materials. The three tweeters cannot be simulated yet, and Assignment 9 fixes that and four other review items before Level 2 M3.
- **Assignment 9** is merged into `main` (merged by Claude on 2026-11-10). The five review fixes are in. The three tweeters simulate from printed sensitivity. The page asks for `baffle` radiation. CHK-FDTD-3 passes. The new outer shapes (`rounded`, `chamfered`, `sloped`, `prism`, `tube`) are registered but incomplete: they report an air volume only, and the simulator still places mounts and the baffle edge as if every cabinet were a box. Assignment 10 completes them.
- **Assignment 10 item 1** (the baffle-step shelf fix) is merged into `main` (merged by Claude on 2026-11-10). Cursor then went inactive. Claude now builds the rest directly.
- **Laptop:** `./start.sh` has still not been run on the owner's Linux Mint laptop. Assignment 2 was done on a Cursor cloud agent.
- **Agent coordination:** Claude assigns via `COMS.md`; Cursor executes assignments and posts handoffs. The owner asked Cursor to follow all of Claude's requests on the board, current and future.
- The specification was rewritten on 2026-11-09, when the owner redefined the product as a speaker design simulator (README section 2).
- Later on 2026-11-09 the owner widened it: the cabinet's outside may be any novel shape, and the lab must take novel radiators such as rear-facing DML panels. The README gained extension points (3.3), open outer shapes and mounts (6.2), radiators as a family with surface sources (5.6), DML panels (5.9), a simple room estimate (5.10), correction feature C10, and the checks CHK-EXT-1, CHK-EXT-2, CHK-RAD-1, CHK-PLATE-1, CHK-ROOM-1.
- The earlier work is kept on the `legacy` branch: a native Android shell that played silence from a Rust engine. It is not this product. Do not port from it.
- An earlier browser prototype existed outside this repository. It is not a reference for this build.

## How the work is done

- From 2026-11-10 Claude writes the code itself. Cursor is out of credits and inactive; its past assignments are in `docs/coms-archive.md`. There is no budget for usage beyond the owner's existing subscriptions.
- Claude runs the full check suite in its cloud workspace with `tools/cloud-check.sh`: the same steps as `./check.sh`, with the standard library built from source for WebAssembly (rustup cannot download the wasm32 target there) and the preinstalled Chromium. The owner never runs that script; `./start.sh` and `./check.sh` stay the owner's and the laptop's.
- If a helper session joins, Claude posts its assignment in `COMS.md` and reviews and merges its branch, as with Cursor.
- When the laptop is linked, Claude can run `./start.sh` there and look at the running app in the Claude app's built-in browser.

## Next steps

1. Claude finishes Assignment 10 directly (cost and the printer record are done): the shape contract for `box` and `tube` (the other four shapes warn instead of faking a box), and the exterior block test.
2. Then the rest of Level 2 M3: `sheet_box`, the other four shapes, recesses and bolt holes, ports, print splitting, and CHK-FDTD-4. Then M4.
3. The owner runs `./start.sh` once on the laptop. This closes the last Level 0 item.

## What has been verified

The full history is in `docs/status-history.md`. The latest state:

- 2026-11-10, Claude's cloud workspace: `tools/cloud-check.sh` ran every step of `./check.sh` on `main` after the shelf fix: rustfmt, clippy, 62 engine tests, the TypeScript bindings, both WebAssembly modules, the type check, vitest, the production build, and all 7 browser tests. This is the first full check outside a Cursor agent. It used stable Rust 1.97.0, not the pinned 1.99.1, and Chromium build 1243 linked as the 1254 that Playwright expects.
- 2026-11-10, Cursor's shelf fix: at the 95 dB target sealed-test under `baffle` keeps a 2.0 dB shelf. CHK-E2E-1 moved from bass limit 125 Hz, evenness 7.4 dB, clean level 83 dB to 113 Hz, 7.4 dB, 83 dB. A test-only 80 dB target keeps a shelf of at least 3 dB and improves evenness.
- 2026-11-10, Claude, Assignment 10 item 2: the lab now prices a design in dollars, line by line (drivers, printed plastic, stock pieces, extras, amplifier units, a processor unless it is built into a bought board), with price dates, unconfirmed marks, a cut list per stock piece, and print time as a lower bound from the X2D's flow. Before this, no dollar figure existed anywhere; the new tests failed to build on the old code, which had no cost report, and the old tube report always bought one piece. A stereo pair of 1.6 m PVC cuts now buys two pieces, and a 3.2 m cut is an error. CHK-COST-1 for a TCP115-4 tube pair matches the hand calculation. Sealed-test costs at least $53.07 for the pair (its PLA); its test driver and test amplifier have no price. `tools/cloud-check.sh` passed: 66 engine tests and 7 browser tests.
- Never yet run on the owner's laptop: `./start.sh` and `./check.sh`.

## Decisions log

One line per decision: date, decision, reason. Decisions that define the product live in the README, not here.

- 2026-11-09: The starting decisions are in README section 3 with their reasons.
- 2026-11-10: Both WebAssembly modules use a plain C ABI, and `wasm-bindgen` is not used. Reason: the owner's laptop then needs only Rust and Node, with no extra build tool to install and keep at a matching version. README section 3 was updated to match.
- 2026-11-10: The real-time kernel writes the test tone into a fixed 128-frame buffer and exports its address. Reason: a JavaScript array cannot be used as a wasm pointer, so the earlier call produced silence.
- 2026-11-10: `rust-toolchain.toml` pins Rust 1.99.1. Reason: README section 3 says to pin the version used to scaffold.
- 2026-11-10: Cost-first direction (owner): every real catalog record has a `tier` of `budget` or `reference`; budget parts come first. Reason: the lab exists to reach hi-fi cheaply, and reference parts show how close it gets.
- 2026-11-10: Level 2 is built in four milestones (M1 to M4 in `tasks/level-2.md`). Reason: same as Level 1.
- 2026-11-10: Level 1 is built in four milestones (M1 to M4 in `tasks/level-1.md`), each reviewed before the next. Reason: Level 1 is too large to review well in one piece, and each milestone still connects every track.
- 2026-11-10: Vite worker bundles use ES module format. Reason: the engine Web Worker must run in Chrome headless and in production preview; IIFE workers hung with a stale `DataView` on wasm memory.
- 2026-11-10: `web/` uses vitest 3.x instead of 4.x. Reason: npm install failed on vitest 4 with this Node/npm combination in the cloud check environment.
- 2026-11-10: Graphs are plain SVG, not a chart library. Reason: the three Level 1 curves do not need another dependency.
- 2026-11-10: `builtin()` lives in `seatlock-sim`, not `seatlock-core`. Reason: the registry type is in core, and core cannot depend on the crates that register kinds.
- 2026-11-10: The engine does not call `Instant::now` in WebAssembly. Reason: that call panics on `wasm32-unknown-unknown` without a JavaScript clock.
- 2026-11-10: A bad simulation request uses the code `unsupported_request`. Reason: it is not a part in the design file, so it should not share `unsupported_kind`.
- 2026-11-10: The printable solid is built in the browser with manifold-3d. Reason: README section 6.3. The 3D view uses three.js.
- 2026-11-10: The 3D solver reuses two pressure buffers instead of allocating one every step. Reason: the review asked for that, and the release run went from 117 ms to 114 ms.
- 2026-11-10: The ideal bass shape is checked from 10 Hz to 3 kHz, and the filters that are actually used are checked on the whole grid. Reason: above a few kilohertz the voice-coil inductance, not the bass shape, sets the clean level, so no high-pass can meet a 95 dB target at 20 kHz. CHK-EQ-2 still measures cone travel, heat, and amplifier voltage at every frequency.
- 2026-11-10: The listening impulse is the path from amplifier voltage to pressure, with the DSP chain left out. Reason: the real-time kernel runs that chain, and putting it in the impulse as well would apply it twice.
- 2026-11-10: Impulse responses use rustfft 6.4. Reason: the task list allows rustfft, and the impulse is an inverse FFT of the simulated pressure.
- 2026-11-10: A fractional delay is a first-order Thiran allpass after the whole-sample delay. Reason: Level 2 asks for a fractional delay, and a first-order Thiran stays a single biquad with no extra dependency.
- 2026-11-10: A port's inside end is treated as flush with the chamber wall and its outside end as free. Reason: that is the CHK-WG-3 description, and it makes the CHK-BOX-3 effective length.
- 2026-11-10: A uniform duct with a rigid end uses the cotangent formula, not a large finite load. Reason: a finite stand-in is not small enough, so the impedance zeros of CHK-WG-1 would not land.
- 2026-11-10: The baffle-step shelf is reported with the correction plan and is not installed by Apply correction. Reason: a +6 dB shelf would move the sealed-test scores that CHK-E2E-1 checks after that button. The shelf frequency is where on-axis edge diffraction is 3 dB below half space.
- 2026-11-10: C4 delay is the path difference from each source to the listener, with the nearest source at zero. Polarity is the automatic flip already in the simulator. Reason: that is the delay the cabinet geometry creates, and a single front driver stays at zero so the sealed checks do not move.
- 2026-11-10: C3's baffle-step shelf will be installed by Apply correction whenever the request shows the baffle step (`baffle`, or `point` with `step`), within headroom, before C2 and C1. This replaces the earlier choice to only report it. Reason: the app now defaults to `baffle`, where the step is real, and CHK-E2E-1's stored values are regression values only (README section 13).
- 2026-11-10: The engine's own default request stays `point` in `half` space. Reason: the numbered checks name that model, and the app sends `baffle` itself.
- 2026-11-10: An unprinted mounting size is omitted, never stored as 0, and the print export is refused. Reason: README section 9.1. A zero would cut no hole and look like a finished baffle.
- 2026-11-10: A stock tube with no published wall uses the design's `wall_m` and says so. Reason: the SAKRETE page does not print a wall, and inventing one would be a catalog number.
- 2026-11-10: The baffle-step shelf is chosen by trying 0 to 6 dB in 0.5 dB steps and keeping the gain with the smaller shortfall (the larger gain on a tie). Headroom is checked from the bass limit up, because the protective high-pass takes the boost away below it. Reason: checking every frequency down to 10 Hz rejected every gain.
- 2026-11-10: Print parts are split to the X2D's main-nozzle build volume, 256 x 256 x 260 mm, and print time is shown as a lower bound from its 40 mm^3/s maximum flow. Reason: the owner named the printer, and the maker's sheet gives the flow only as a maximum on a thin-walled test print.
- 2026-11-10: PLA is the default print material. Reason: the owner is open to any filament, and PLA has the cheapest price sourced so far ($13.49/kg).
- 2026-11-10: Amplifier units are bought from the design's list: one of each listed model, then more of the first listed model until every radiator in the system has a channel. A processor marked `built_into` an amplifier that is bought costs nothing extra. Tube end caps are flat printed discs of the pipe's outside diameter, 8 mm thick unless the design gives `cap_thickness_m`. Reason: README section 9.2 asks for the electronics that give every radiator a channel, and the KABD-4100's processor is on the same board as its amplifier.
- 2026-11-10: Printing is optional (owner). A cabinet can be all stock, all printed, or a mix, and the lab prints a part only when that improves the sound or lowers the cost. `sheet_box` moved from Level 3 to Level 2, because the cheap-against-reference comparison already needs a conventional box cut from sheets. The README opening and sections 2, 3.3, 6.2, 9.2, and 12 and `tasks/level-2.md` were updated.

## Tool versions

- Rust **1.99.1** pinned in `rust-toolchain.toml` (components `rustfmt` and `clippy`, target `wasm32-unknown-unknown`).
- Node **22.23.4** (engine requirement >= 22.12).
- web: **vite 7.3.8**, **react 19.3.1**, **typescript 5.9.3**, **vitest 3.2.7**, **@playwright/test 1.65.0** (see `web/package-lock.json`).
- ts-rs **10.1.0** (typescript feature on `seatlock-core` only, not wasm builds).

## Questions for the owner

Ask each one when its answer starts to matter. Record the answer here and in the README.

1. **Quality target.** README section 11.2 uses placeholder numbers: bass limit 50 Hz or lower, evenness 1.5 dB or better, maximum clean level 95 dB at 1 m or higher. Are these right for the owner's listening? Needed before the Level 2 optimizer.
2. **Playback hardware for real builds.** What will run the crossover and correction and drive the amplifiers: a phone, a laptop, a small computer, a DSP board? This decides the export formats (README section 7) and the electronics catalog. Needed during Level 2.
3. **Printer.** Answered on 2026-11-10: the owner has a Bambu Lab X2D and is open to any filament. The printer record is `catalog/printers/bambu-lab-x2d.json`: main-nozzle build volume 256 x 256 x 260 mm and a maximum hotend flow of 40 mm^3/s, from the maker's spec sheet (a reseller's copy; bambulab.com did not load). PLA is priced at $13.49/kg (Elegoo PLA Basic, 1 kg), with density 1.24 g/cm^3 and stiffness from Bambu's PLA Basic data sheet. PETG has no price yet.
4. **Protected band.** The rule "no EQ boost from 3 kHz to 5 kHz" was carried over from the owner's earlier notes and is on by default. The earlier rule "no boost below 150 Hz" was replaced by the physics limit of README section 8. Confirm both.
5. **Budget.** Is there a ceiling per pair of speakers? Optional. It becomes `budget_max` in the target.
6. **Rear-facing DML panels.** What will the panels be made of (printed, or bought sheet such as foam board or plywood)? How far from the wall will the speakers stand? Should in-room results aim for a gently falling response rather than a flat one (the target's tilt)? Needed at Level 3.
7. **Hosting.** Should the lab also be reachable as a web link, in addition to running on the laptop? The build is static files, so this is possible at any time.
