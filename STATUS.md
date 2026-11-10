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
- **Laptop:** `./start.sh` has still not been run on the owner's Linux Mint laptop. Assignment 2 was done on a Cursor cloud agent.
- **Agent coordination:** Claude assigns via `COMS.md`; Cursor executes assignments and posts handoffs. The owner asked Cursor to follow all of Claude's requests on the board, current and future.
- The specification was rewritten on 2026-11-09, when the owner redefined the product as a speaker design simulator (README section 2).
- Later on 2026-11-09 the owner widened it: the cabinet's outside may be any novel shape, and the lab must take novel radiators such as rear-facing DML panels. The README gained extension points (3.3), open outer shapes and mounts (6.2), radiators as a family with surface sources (5.6), DML panels (5.9), a simple room estimate (5.10), correction feature C10, and the checks CHK-EXT-1, CHK-EXT-2, CHK-RAD-1, CHK-PLATE-1, CHK-ROOM-1.
- The earlier work is kept on the `legacy` branch: a native Android shell that played silence from a Rust engine. It is not this product. Do not port from it.
- An earlier browser prototype existed outside this repository. It is not a reference for this build.

## How the work is done

- Claude directs and Cursor builds. Claude writes each piece of work as a task file in `tasks/`, reviews what Cursor produced against this README, and writes the next task or the fixes.
- Cursor runs as a Cursor cloud agent for now, by the owner's choice on 2026-11-10. The laptop (Linux Mint, 32 GB RAM) will be set up later. There is no budget for usage beyond the owner's existing subscriptions.
- When the laptop is linked, Claude has a shell on it, so Claude runs `./start.sh` and `./check.sh` there itself.
- Through computer use, Claude can watch Cursor on the laptop and click its buttons, but cannot type into Cursor or a terminal. The owner relays one line per round to Cursor's Agent chat: `Check COMS.md`.
- Claude can open the running app on the laptop in the Claude app's built-in browser to test it.
- Claude's own cloud workspace cannot download Rust's WebAssembly target, because the host `static.rust-lang.org` is blocked there. It can run the native tests only. Cursor's machines and the laptop have no such limit.

## Next steps

1. Cursor does Assignment 9 in `COMS.md`: five review fixes from Assignment 8, then Level 2 M3 (outer shapes including `tube`, hybrid builds, print parts, and the 3D solver with openings). Claude reviews it before M4.
2. The owner runs `./start.sh` once on the laptop, and Claude checks the running app there in the built-in browser. This closes the last Level 0 item.

## What has been verified

- **`./check.sh` on 2026-11-10 (Cursor cloud, UTC):** rustfmt, clippy, `cargo test --workspace`, ts-rs export test, release wasm builds for `seatlock-wasm` and `seatlock-rt`, TypeScript check, vitest, production build, Playwright headless test (engine version + example design load).
- Design `sealed-test` round-trips through `design.check` in native tests; unknown radiator kinds warn and still round-trip (CHK-EXT-2 early).
- Claude reran the native tests and clippy on the merged code in its own workspace on 2026-11-10: they pass. Claude cannot build WebAssembly there.
- Claude's review of Level 0 on 2026-11-10 found the silent test tone. The Playwright test at that time checked the engine and the design round-trip only.
- 2026-11-10, before the tone fix, on a Cursor cloud agent: the new worklet check rendered 0.5 s and measured RMS 0.
- 2026-11-10, after the fix, same machine: that check passed (RMS above 0.05, dominant frequency within 2 Hz of 440 Hz). `./check.sh` was run here, not on the owner's laptop.
- 2026-11-10, Level 1 M1 on a Cursor cloud agent: CHK-DRV-1, CHK-DRV-2, CHK-BOX-1, CHK-BOX-2, CHK-DSP-1, CHK-DSP-3, CHK-EXT-1, and CHK-EXT-2 pass. `simulate` of sealed-test takes about 1 ms natively. WebAssembly has no clock here, so the engine reports 0 ms in the browser; the native timing is the one recorded.
- 2026-11-10, Level 1 M2 on the same cloud agent: CHK-DRV-2, CHK-BOX-1, and CHK-BOX-2 also pass through `simulate` on 5.000 L fixtures. CHK-FDTD-1, CHK-FDTD-2, CHK-GEO-1, CHK-GEO-2, and CHK-COST-1 pass. The sealed-test box at 10 mm cells took 2.2 s.
- Claude reran all engine tests, clippy, and rustfmt on the M1 branch in its own workspace on 2026-11-10: all pass. Review note: the section 13 checks call physics helpers directly, not `simulate`; Assignment 4 adds the same checks through `simulate`.
- Claude reran all engine tests (release build), clippy, and rustfmt on the M2 branch in its own workspace on 2026-11-10: all pass, including CHK-FDTD-1, CHK-FDTD-2, CHK-COST-1, and the section 13 checks through `simulate`.
- Claude's M2 review found that `run_grid` calls `std::time::Instant::now()` unconditionally. That call panics on `wasm32-unknown-unknown`, so the in-app 3D solver run fails in the browser. No browser test covered it.
- 2026-11-10, before the clock fix, the new browser test waited for the 3D curve and it never appeared.
- 2026-11-10, after the fix, same cloud agent: the browser test draws the curve, cancel returns the page to idle, and a second run works. Sealed-test at 10 mm warns that the grid's air volume differs from the box. The exact CHK-FDTD-2 box does not warn. Release timing of that 10 mm run was 117 ms before swapping the pressure buffers and 114 ms after. The earlier 2.2 s figure was a debug build.
- Claude reran all 27 engine tests (release), clippy, and rustfmt on the M3 branch on 2026-11-10: all pass.
- Claude ran correction on sealed-test and on the 5 L fixture on 2026-11-10. Both returned only a high-pass at 80 Hz and the message "No filter improved the response enough to keep". Flatness stayed at 6.44 dB. Causes are listed in Assignment 6: C1 stops at the first region it may not touch, its target is flat below `f_low`, and C2 only adds a high-pass instead of extending the bass.
- 2026-11-10, Assignment 6 on a Cursor cloud agent: the new rolloff-plus-peak check failed on the old C1 with "No filter improved the response enough to keep", then passed after C1 learned to skip that region. The 5 L limit check then failed on cone travel (4.05 mm at 44 Hz) because a bass limit of 71 Hz still boosted, and the sealed-test headroom check failed on a 0.47 dB boost where the driver had no headroom. Both pass after the bass limit is chosen from the filters that are actually used. Woofer B in 20 L at 85 dB extends the bass and stays inside its limits. CHK-EQ-1, CHK-EQ-2, and CHK-EQ-3 pass. `./check.sh` passed, including CHK-E2E-1. It was not run on the owner's laptop.
- The 10 mm solver time for sealed-test is about 115 ms in a release build on the Cursor cloud agent. The earlier 2.2 s figure was a debug build.
- 2026-11-10, Assignment 7 on a Cursor cloud agent: CHK-BOX-3, CHK-BOX-4 (directly and through `simulate`), and CHK-DSP-2 pass. Three sourced drivers match their printed moving mass, compliance, and force factor within 10 %. `./check.sh` passed. It was not run on the owner's laptop.
- Datasheets opened on 2026-11-10 and copied into the catalog: Dayton Audio TCP115-4, PS95-8, and RS100-8, plus Parts Express pages for mounting dimensions and prices ($15.98, $29.98, and $35.98). Tweeter sheets that were opened (ND25FA-4 and DC28F-8) print Vas, Xmax, Mms, and Cms as N/A, so those drivers were not added.
- Claude reran all 34 engine tests (release), clippy, and rustfmt on the Assignment 6 branch on 2026-11-10: all pass. Correction on sealed-test now gives a Linkwitz transform (84 Hz to 85 Hz) and a protective high-pass at 42 Hz, keeps no C1 filter, and explains why. The lumped `le_h` inductance model makes the high treble fall steeply, which limits evenness (6.5 dB) and maximum clean level (83 dB) for the test drivers. Published response curves at Level 2 replace that region.
- 2026-11-10: README section 5.6 gained the edge diffraction model for `baffle` radiation, section 13 gained `test-tweeter-a` and CHK-RAD-2 (-6.02 dB at 10 Hz, +3.52 dB at 1716 Hz, -6.02 dB at 3432 Hz for a 0.10 m circular baffle; computed from the model's formula).
- Claude reran all 40 engine tests (release), clippy, and rustfmt on the Assignment 7 branch on 2026-11-10: all pass. Claude checked the TCP115-4 record against a copy of the Dayton specification sheet: every Thiele–Small value matches as printed.
- 2026-11-10: the owner set the product direction: hi-fi from cheap, new, off-the-shelf parts and materials with public data, with quality parts and conventional materials as a reference. Used, salvaged, or untested parts are out of scope. README sections 2, 6.2, 9, 12, 15, and 16 and `tasks/level-2.md` were updated.
- 2026-11-10: README section 5.1 gained a derivation path for drivers whose sheets do not print `vas_m3` (from `mms_kg` or `bl_tm`), and a rule for drivers without `xmax_m`. Reason: two tweeters were left out of the catalog for lack of Vas.
- The reference numbers in README section 13 were computed on 2026-11-09 with a throwaway Python implementation of the README's equations. It is not in the repository. It covered the driver model, sealed and vented `lumped` boxes, the enclosure load interface, a `waveguide` port and resonator, biquad and crossover sums, and a small `fdtd` solver with a resistive source, semi-implicit boundaries, and an absorbing exterior block.
- That solver reproduced box resonances within 0.2 % and a Helmholtz resonance within 0.2 % of the `lumped` value. An explicit boundary update went unstable, which is where the rule in README section 5.5 comes from.
- CHK-FDTD-4 and the time budgets in section 5.5 are estimates. Nothing has measured them yet.
- CHK-PLATE-1 is the closed-form result, cross-checked with a finite-difference eigenvalue solve (within 0.1 %). CHK-RAD-1 and CHK-ROOM-1 were computed from their formulas.
- Not validated by anything yet: the exciter and backing equations of section 5.9, the `exterior` radiation model and its reciprocity run (5.6), the `baffle` model's edge diffraction, and the `simple` room's reflection term. Treat them as starting designs.
- 2026-11-10, Assignment 8 on a Cursor cloud agent: CHK-WG-1, CHK-WG-2, CHK-WG-3, CHK-RAD-1, and CHK-RAD-2 pass. A `baffle` simulation of sealed-test at 30 Hz is about 6 dB below the half-space point model. The first absolute comparison was 56 dB low because the baffle formula used `k` where the point model uses `omega`; the relative baffle checks had still passed. `./check.sh` passed on this cloud agent. It was not run on the owner's laptop, and `./start.sh` was not run there.
- Catalog gaps for Assignment 8, all from pages opened on 2026-11-10. ND25FA-4, DC28F-8, and RST28F-4 are in the catalog, but their sheets print moving mass, compliance, force factor, Vas, and Xmax as N/A, so the engine refuses to invent a cone model and warns. Those three have no price and no mounting, because the product pages did not load. TCP115-8 and RS225-4 have full Thiele–Small numbers from their sheets and no price or mounting, because those product pages did not load. ND65-4's bolt circle was not on the page. A direct fetch of the ND65-4 product page returned an empty body; its price ($23.98) and mounting came from a search extract of that same URL. DSA135-8 ($39.98) and PC105-8 ($17.79) prices and mounting also came from search extracts of their Parts Express pages; their specification sheets were fetched directly. PC105-8 Vas is 0.12 cubic feet on the sheet (stored as 0.003398 m³). ND65-4's printed moving mass differs from the Vas derivation by more than 10 percent, so the simulation keeps the derived value and warns. KABD-430's own product page did not load: 4 × 30 W is from the KABD-4100 user guide, and $64.98 was on the KABD-4100 similar-items list. Its rated load was not printed. KABD-4100 does not print a sample rate or a biquad count. No seller page printed density or stiffness for the PVC pipe, concrete-form tube, foam board, MDF, or plywood, so those fields are empty and any result that needs them cannot be computed yet. The plywood price ($70.55) is from the Home Depot sanded-plywood category listing for the SANDEPLY 18 mm sheet; the product page gave the actual size 0.709 × 48 × 96 in and did not show the price in the extracted text. Sheets that returned no parameters and were not added: ND105-4, ND91-4, RS125-8, RS150-4, RS150-8, and RS180-8. No measured response file was copied onto a real driver.
- Claude reran all 51 engine tests (release), clippy, and rustfmt on the Assignment 8 branch on 2026-11-10: all pass.
- Claude's Assignment 8 review found: (1) six driver records store 0 for mounting sizes their pages did not print, and RS225-4 and TCP115-8 carry bolt counts that no opened page gave for those models; (2) the three tweeters store no sensitivity, so none of them can be simulated; (3) the app asks for `point` radiation in `half` space, which hides the baffle step, and C3's shelf is never installed; (4) several prices and mountings came from search extracts or another product's listing with nothing in the record saying so; (5) stock records have no density or stiffness.
- 2026-11-10: README section 5.1 gained a third derivation path, from printed sensitivity. Claude checked it by inverting the CHK-DRV-2 level: test-woofer-a at 87.75 dB for 2.83 V gives back Vas 10.005 L against 10 L, and the half-space constant is 112.16 dB.
- 2026-11-10: README section 9.1 now forbids writing 0 or a sibling model's value for an unprinted field, and marks figures not read on the part's own page with `via`. Section 14 makes `baffle` the app's default radiation model.

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

## Tool versions

- Rust **1.99.1** pinned in `rust-toolchain.toml` (components `rustfmt` and `clippy`, target `wasm32-unknown-unknown`).
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
