# Agent communications

The handoff board between sessions (README section 1.4). Every chat starts with no memory of earlier chats, so whatever the next session needs is written here. The owner does not need to read this file.

Read this file at the start of every session, immediately after `README.md` and `STATUS.md`. Keep messages short, dated, and actionable.

## Standing rules: what to do on "Check COMS.md"

The owner starts a session by typing **Check COMS.md**. Every instruction lives in this file.

**Lead session (Claude), every time:**

1. Fetch every branch and pull `main`. A fresh clone may fetch `main` only, so first run `git config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'`, then `git fetch origin`.
2. If a helper's handoff is waiting for review on its branch: review it against the README, rerun the checks, merge it into `main` or not, and post the review here.
3. Otherwise take the newest lead handoff at the top of the active thread and continue its work queue from the first item not done. Do not ask the owner whether to continue. Ask only the product questions that handoff names, when they start to matter.
4. Before stopping for any reason: make `STATUS.md` true, post a new handoff at the top of the active thread (what is done, what is not, the next queue, the tail of the checks), commit, and push to `main`.
5. Questions only the owner can answer go in the chat reply, one at a time with a recommendation, and into `STATUS.md`.

**Helper, every time** (only when the lead has posted an assignment for one):

1. Fetch and pull `main`. Read README, STATUS, and this file. Find the newest assignment addressed to you.
2. If it holds work you have not done: post an ack, then work on a new branch from current `main`. Push commits as you go.
3. Before you stop for any reason, push your branch and post a handoff at the top of the active thread on that branch. List every item as done, partly done, or not done. Give the tail of the checks and the machine they ran on.
4. If your handoff is waiting for review, or nothing new is addressed to you: change nothing, and tell the owner "Nothing new for the helper; waiting for the lead."
5. Never merge into `main` and never delete branches. The lead merges.
6. If the README is silent or wrong on something that affects more than one track, post a message with Status `question` or `blocked`, then keep going on work that does not depend on the answer.
7. In `STATUS.md`, add lines or change only the lines your work affects. Never rewrite or delete its history.
8. End your chat reply with two or three plain sentences for the owner: what now works, and what happens next.

## How to write here

Append new entries **at the top** of the thread below (newest first). Use this block:

```
### YYYY-MM-DD — Author — Subject

- **To:** next lead session | helper | both
- **Status:** question | answer | handoff | FYI | blocked
- **Context:** one line (branch, level, track if relevant)

Body: what you need or what you did. End with explicit next step for the other agent, or "none".
```

When a thread is done, add one line under **Resolved** at the bottom with the date and one-sentence outcome. Do not delete old messages; when the board grows long, move older threads unchanged to `docs/coms-archive.md`.

## Active thread

### 2026-11-11 — Claude — Handoff and brief: Level 3 physics for the Claude Fable 5.2 session

- **To:** next lead session (the owner plans to use Claude Fable 5.2, with only a few hours)
- **Status:** handoff
- **Context:** `main`. Level 2 complete (M1 to M4). Level 3 starts. No helper is active.

**Start here (keep it short; the hours are limited)**

1. Fetch every branch (standing rule 1); nothing waits for review. Run `tools/cloud-check.sh` in the background (about 6 minutes; last run on this commit: 106 engine tests, 20 browser tests, all passing) and start reading while it runs.
2. Read README 3.3, 5.6, 5.9, 5.10, 13 (CHK-PLATE-1, CHK-ROOM-1). The pitfalls list in the previous handoff below still applies.
3. Work in this order, the owner's choice. Commit and push after each item; each item stands alone.

**Item 1: the `simple` room (README 5.10, CHK-ROOM-1).** Closed form, quick.
- Contract today: `RoomModel::at_listener(&self, direct: &[Complex64])` in `engine/crates/seatlock-core/src/registry.rs`, member `free` registered in `seatlock-acoustics/src/lib.rs`, used per radiator in `simulate` (`room.at_listener(&pressure)` in `seatlock-sim/src/lib.rs`). It cannot see the speaker's position, its sound power, or the wall. Extend it (keep `free` working): the room needs, per radiator, the pressure toward the wall behind (mirror source at `2 * wall_behind_m` extra path) and the radiated sound power `W`. The design field already exists: `speaker.room` (`Option<KindPart>`).
- Energy sum: direct and reflection add as complex pressures, diffuse adds as energy (README 5.10), so the summed in-room result is a magnitude, not a complex curve; keep the complex direct sum for listen mode and give the in-room level separately.
- CHK-ROOM-1: A = 20.12 m^2, diffuse 113.12 dB, direct at 2.5 m 101.19 dB.

**Item 2: `exterior` radiation by reciprocity (README 5.6).** The hard one.
- Base: `seatlock_fdtd::grid::{Grid, run}` (masked grid, dense loops, spherical-wave absorbing faces) and the voxelizer pattern in `seatlock-sim/src/voxels.rs` (`Lattice`, per-axis shift, `OuterShape::contains_cabinet` for the outer body). Every shape implements `contains_cabinet`.
- `run` today drives a resistive source patch and records the aperture; it needs a hard point source (or an incident plane wave) and recorded pressure on the cabinet's skin cells. A listener 1 m away inside the grid is about 8 million cells at 10 mm; consider a plane wave from the listener's direction (total-field/scattered-field) and the far-field 1/r, or a Kirchhoff surface, and record the choice in STATUS.
- Contract: `RadiationModel::pressure` takes `sources: &[Vec<Complex64>]` with no positions; the `baffle` path in `simulate` builds positioned sources itself. Give `exterior` positioned surface sources (position, outward normal, area, volume velocity per frequency) and keep `point` and `baffle` working.
- Validate before trusting (README: "nothing has validated it yet"): a flat-baffled source against CHK-RAD-2's closed form, low-frequency full-space level, and reciprocity (swap source and receiver). Valid up to `C0 / (10 dx)`; above it fall back to `baffle` and say so in the fidelity label.

**Item 3: the DML panel (README 5.9, CHK-PLATE-1).** New radiator kind `dml_panel` in `seatlock-acoustics` (register beside `cone`), panel modes by finite differences (free edges have no closed form; CHK-PLATE-1 is the simply supported closed form: 42.36, 88.10, 123.68, 164.35, 169.43 Hz for `test-panel-a`), the exciter coupling of 5.9, surface sources per element (needs Item 2's positioned sources). Use `test-panel-a` and a synthetic `test-exciter-a`; do not spend Fable's hours sourcing catalog records (that is ordinary Level 3 track B work).

**Item 4, if time remains:** losses in the 3D model (fill and wall), then the two 3D wave views (Level 3 track D). Otherwise leave them for a regular session.

**State of Level 2 work worth knowing**

- Scoring: `seatlock-sim/src/compare.rs` (`score`, `apply_correction`, `rebuild_to`, `build_methods`, `reference_build`); optimizer `seatlock-sim/src/optimize.rs`; page panel `web/src/CostQuality.tsx` on the generic `web/src/jobWorker.ts`; wasm methods `compare.reference`, `compare.methods`, `optimize.combos`, `optimize.combo`.
- Owner questions: 1 answered (keep the quality target). 2 (playback hardware) asked on 2026-11-11; record the answer in STATUS and README 7 when it comes.
- Open items, not for Fable: 10 mm full 3D preview of a 20 L ported cabinet takes 26 s in the browser (target 20 s); the vented self-check reads 5.1 %; the "Openings are not part of the sealed-box model yet" warning appears for every design with ducts; `waveguide` for several chambers; bent ports in print; the optimizer cannot reshape prisms or multi-chamber cabinets; the SAKRETE tube needs a wall before it can be a build method.

**Next step:** Item 1.

### 2026-11-10 — Claude — Handoff: start here. Assignment 12 (Level 2 M4)

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 2: M1, M2, and M3 done; M4 not started. No helper is active.

**First steps**

1. Read README sections 1 to 4, 9.2, 11, and 12. Then STATUS and this post.
2. Fetch every branch (standing rule 1). Nothing is waiting for review.
3. Run `tools/cloud-check.sh`. Last run, 2026-11-10 on `main` at this post's commit: rustfmt, clippy, 100 engine tests (CHK-FDTD-4 takes about 90 s), bindings, both wasm modules, type check, vitest, build, 18 of 18 browser tests (about 45 s).

**Done this session (Assignment 11 item 5; M3 complete)**

- `seatlock-fdtd/src/grid.rs`: one general solver on a masked grid (dense loops over 0/1 face masks; face slot = index of the cell on the high side), source patch, exterior blocks with spherical-wave absorbing faces, flow per opening, DFT at requested frequencies, progress callback and cancel. `EXTERIOR_WIDTHS = 5`. The block-doubling test now passes (ignored only because it takes 75 s; run with `--ignored`).
- `seatlock-sim/src/voxels.rs`: grid for any builder cabinet (shape air, chambers, bores from `ports::bores`, tube walls closed, driver body, per-axis shift, leak closing). `simulate` with `enclosure: "fdtd"` and `fdtd_dx_m` (`full_3d_enclosure` in `lib.rs`); result field `fdtd` (`FdtdReport`). Contract gained `contains_cabinet`.
- wasm method `simulate.full3d` with a `seatlock_progress` import; `web/src/fdtdWorker.ts` runs 10 mm then 5 mm; `FullThreeD` in `App.tsx` shows both graphs and the two-grid difference.
- `engine/.cargo/config.toml`: `+simd128` for wasm32.
- CHK-FDTD-4 in `seatlock-sim/tests/full_3d.rs` (fixture `chk-fdtd-4`): 1.4 % tuning, 0.35 dB.

**Open items from item 5** (not before M4 unless cheap)

- 10 mm preview of a 20 L cabinet with a port takes 26 s in the browser against the 20 s target. The exterior block is most of the cells; a half-sphere block, or a block only on the open side, may halve it, but the doubling rule must be rechecked for any new block shape.
- The vented self-check reads 5.1 % at 5 mm; it carries a warning that explains the ports' end corrections. Consider comparing a sealed variant instead.
- `simulate` warns "Openings are not part of the sealed-box model yet" for every design with ducts, even though `lumped` handles ports. Pre-existing; remove or reword it.
- Earlier follow-ups still stand: `waveguide` for several chambers, printing bent ports, showing the print cuts in the 3D view.

**Assignment 12: Level 2 M4** (`tasks/level-2.md` M4): the optimizer (README 11.3), cost beside every score, the cheap-against-reference comparison (README 9.2), the side-by-side build methods (README 6.2), and CHK-E2E-2 with `two-way-vented`. STATUS question 1 (quality target) was put to the owner at the end of this session with the recommendation to keep the placeholders; if the answer is in the chat or STATUS, record it in `targets/hifi-default.json`, README 11.2, and STATUS before the optimizer depends on it. Question 2 (playback hardware) is due during Level 2; ask it after question 1 is answered, one at a time, with a recommendation. Build the parts that do not depend on the target first (cost beside every score, cheap against reference, side-by-side build methods).

**Then Level 3, and Fable.** Unchanged: when Level 2 is complete, tell the owner it is time, and post the Fable brief here first (exact files, checks, order; `exterior` radiation and the `simple` room first, then the DML panel; losses in the 3D model; the two 3D wave views). The new general grid (`seatlock_fdtd::grid::Grid` and `run`) and the spherical-wave absorbing faces are the natural base for `exterior` radiation by reciprocity.

**The owner and the laptop**

- The owner is not a programmer (README 1.1). Report result first, in plain language, without file names.
- The laptop is linked to the Claude app as `nathan-latitude-5521`, but these chats get file access only, no terminal. The owner runs commands when asked; `start.sh` works there. The full 3D run times have not been measured on the laptop.

**Pitfalls in the cloud workspace**

- Rust is the installed stable (1.97.0), not the pinned 1.99.1. Set `RUSTUP_TOOLCHAIN=stable` for direct `cargo` calls; run `cargo fmt --all` from `engine/`.
- Use `tools/cloud-check.sh`, not `./check.sh`. Browser tests run against the production build: after changing `web/src`, run `npm run build` before rerunning one spec with `cd web && PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/seatlock-cloud/pw npx playwright test e2e/<file> -g "<name>"`.
- Do not run `npx prettier` on `web/src`: the repository is not formatted with it, and it rewrites whole files.
- In browser tests, avoid one `expect()` per mesh element.
- The engine must not call `Instant::now` in WebAssembly; the worker times wasm runs with `performance.now()`.
- CHK-E2E-1's stored values are regression values only. Change them only with a reason in STATUS (last: $53.06, bolt holes).

**Next step:** Assignment 12.

### 2026-11-10 — Claude — Handoff: start here. Assignment 11 item 5 (3D model with openings), then Assignment 12 (M4)

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 0 complete (owner ran `start.sh`). Level 2: M1 and M2 merged; M3 done except item 5; M4 not started. No helper is active.

**First steps**

1. Read README sections 1 to 4 and 12, plus 5.5 and 6. Then STATUS and this post.
2. Fetch every branch (standing rule 1). Nothing is waiting for review.
3. Run `tools/cloud-check.sh` (first run in a new workspace fetches std sources and runs `npm ci`; allow several minutes). Last run, 2026-11-10 on `main` at this post's commit: rustfmt, clippy, 96 engine tests (plus the one ignored known failure below), bindings, both wasm modules, type check, vitest, build, 16 of 16 browser tests.

**Done this session (Assignment 11 items 1 to 4)**

- Shapes: `rounded`, `chamfered` (convex hulls, `seatlock-geom/src/edged.rs`), `sloped` and `prism` (`seatlock-geom/src/prism.rs`, CSG op `prism`, plus op `hull`). Examples `rounded-box`, `chamfered-box`, `sloped-baffle`, `hex-prism`.
- Recesses and bolt holes: `mount_features` in `seatlock-sim/src/lib.rs`.
- Ports and ducts: `seatlock-sim/src/ports.rs` (straight only; tapered round, or slot). Chambers: `seatlock-sim/src/chambers.rs` (boxes; partitions = `air_tree` minus chambers) and `seatlock-acoustics/src/network.rs` (lumped network). Example `two-chamber`. `two-way-vented`'s port path fixed (STATUS decisions).
- Print splitting: `seatlock-sim/src/split.rs` (cuts and pins), `buildPrint` in `web/src/printSolid.ts` (parts and 3MF with one object per part and pin), parts table in `App.tsx` (`data-testid="print-parts"`).
- Contract additions on `OuterShape`: `air_tree`, `contains_solid` (default "not finished yet").

**Assignment 11 item 5: full 3D (`fdtd`) for builder cabinets with openings** (README 5.5). Unchanged from the previous post, now unblocked:

- Build the voxel mask from the shape's `contains_air` (and the chambers, via `chambers::Chamber::contains`). Today the page sends a rectangular inner size from `web/src/App.tsx`, and the button is disabled for every shape except `box`.
- Put the source at the mount. Each duct to outside is an opening into an exterior air block with absorbing faces; `H_k` from the flow through it. Generalize `helmholtz_neck` in `engine/crates/seatlock-fdtd/src/lib.rs`. The printed port's geometry is in `ports::PrintedPort` (bore and opening).
- Two-resolution uncertainty (10 mm and 5 mm), progress from the worker, cancel.
- CHK-FDTD-4: 5 mm against `waveguide` for the CHK-BOX-3 cabinet as the builder makes it. Tuning within 3 %, SPL within 1.5 dB below 300 Hz.
- Known failure, ignored on purpose: `exterior_block_doubling_moves_resonance_and_off_resonance_values_under_one_percent` in `engine/crates/seatlock-fdtd/tests/checks.rs`. Do not loosen it; validate a decay window against the closed form, or leave it to Level 3 losses, and record the choice in STATUS.
- Record measured run times against the budgets in README 5.5.
- If this stalls badly, the owner may choose to spend Claude Fable 5.2 on it instead; offer that, do not decide it.

**Smaller follow-ups found this session** (do when convenient, not before item 5): `waveguide` for several chambers (tree networks, README 5.4); printing bent ports; showing in the 3D view where the print is cut.

**Assignment 12: Level 2 M4** (`tasks/level-2.md` M4): the optimizer (README 11.3), cost beside every score, the cheap-against-reference comparison (README 9.2), the side-by-side build methods (README 6.2), and CHK-E2E-2 with `two-way-vented`. Before the optimizer depends on it, ask the owner STATUS question 1 (quality target). Question 2 (playback hardware) is also due during Level 2. One question at a time, with a recommendation.

**Then Level 3, and Fable.** Only a few hours of Fable are left (owner, 2026-11-10), so the brief must let that session start coding at once: exact files, checks, and order, with nothing left to read up on. Order, confirmed by the owner: `exterior` radiation and the `simple` room first (needed to judge the rear-facing DML panels), then the DML panel. When Level 2 is complete, tell the owner it is time, and post that brief here first: `exterior` radiation (README 5.6), the DML panel and exciter (5.9, CHK-PLATE-1), the `simple` room (5.10, CHK-ROOM-1), losses in the 3D model, and the two 3D wave views (Level 3 track D).

**The owner and the laptop**

- The owner is not a programmer (README 1.1). Report result first, in plain language, without file names.
- The laptop is linked to the Claude app as `nathan-latitude-5521`, but these chats get file access only, no terminal. The owner runs commands when asked; `start.sh` works there.

**Pitfalls in the cloud workspace**

- Rust is the installed stable (1.97.0), not the pinned 1.99.1. Set `RUSTUP_TOOLCHAIN=stable` for direct `cargo` calls; run `cargo fmt --all` from `engine/`.
- Use `tools/cloud-check.sh`, not `./check.sh`. To rerun one browser test: `cd web && PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/seatlock-cloud/pw npx playwright test e2e/<file> -g "<name>"`.
- In browser tests, avoid one `expect()` per mesh element; Playwright's per-call overhead adds up to tens of seconds.
- The engine must not call `Instant::now` in WebAssembly.
- CHK-E2E-1's stored values are regression values only. Change them only with a reason in STATUS (last: $53.06, bolt holes).

**Next step:** Assignment 11, item 5.

### 2026-11-10 — Claude — Handoff: start here. Assignment 11 (rest of Level 2 M3), then Assignment 12 (M4)

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 2: M1 and M2 merged, M3 partly done, M4 not started. No helper is active; the lead commits to `main` directly.

**First steps**

1. Read README sections 1 to 4 and 12 at least, plus 5.5 and 6 for this assignment. Then STATUS and this post.
2. Fetch every branch (standing rule 1). Nothing is waiting for review.
3. Run `tools/cloud-check.sh`. In a new workspace its first run fetches the standard library sources for the installed compiler from GitHub and runs `npm ci`, so allow several minutes. Last run, 2026-11-10 on `main` with this post's commits: rustfmt, clippy, 69 engine tests passed (one more is ignored as a known failure, below), the TypeScript bindings, both WebAssembly modules, the type check, vitest, the production build, and 9 of 9 browser tests.
4. Work the queue below in order. Small commits, each passing `tools/cloud-check.sh`, pushed to `main` as you go. For every new check, prove it fails before the code it tests exists or is fixed.

**State in brief**

- Done in M3: `box`, `tube` (PVC pipe with printed end caps, a hybrid build), and `sheet_box` (cut from MDF or plywood, with a cut list and whole sheets in the cost) answer the whole outer-shape contract (`OuterShape` in `engine/crates/seatlock-core/src/registry.rs`). The print solid is a CSG tree from the engine, built on the page with manifold-3d. STL and 3MF export. Cost in dollars, line by line, with price dates and print time as a lower bound from the X2D's flow. CHK-FDTD-3 passes. The exterior-block rule of README 5.5 was measured and fails (below).
- Examples: `sealed-test`, `two-way-vented`, `pvc-tube` (TCP115-4 in 4 in. PVC), `mdf-box` (TCP115-4 in 3/4 in. MDF).
- Not done in M3: Assignment 11 below.

**Assignment 11: the rest of Level 2 M3** (README 5.5 and 6.2 to 6.4; `tasks/level-2.md` M3)

1. **Finish `rounded`, `chamfered`, `sloped`, and `prism`** through the whole outer-shape contract, as `BoxShape` and `TubeShape` do in `engine/crates/seatlock-geom/src/lib.rs`: a mount on every named surface, outlines for edge diffraction, `contains_air`, print volume, and the print tree. Add a CSG primitive if a shape needs one (for example an outline extruded along an axis), in the engine's tree and in `web/src/printSolid.ts`. Today these four answer only the air volume, so the simulator says "not finished yet"; the test `an_unfinished_shape_says_so_and_gives_no_curves` covers all four, so take each kind out of it as you finish it. Known errors to fix: `prism` subtracts only the end walls from its volume (the inside outline must be the outer one moved inward by `wall_m`), and `rounded` ignores the rounding. Each finished shape gets an example design, CHK-GEO-1 (formula against print solid within 0.5 %, voxels at 5 mm within 2 %), and CHK-GEO-2, through `web/e2e/geometry.spec.ts` (helpers `checkExportedStl` and `checkVolumes`).
2. **Recesses and bolt holes** from the driver's `mounting` record (`outer_diameter_m`, `bolt_circle_m`, `bolt_count`) and the mount's `recess_m`, in the print tree. A size the record does not print refuses the export, as the cut-out does now (README 9.1).
3. **Ports and chambers in the builder.** A duct to `outside` is simulated today (`lumped` and `waveguide`, placed as a source by `port_poses` in `seatlock-sim`), but the print tree does not cut or build it. Cut it through the wall and print the port as a round tube or a slot, from the duct's `section`. `chambers` is parsed but read by nothing yet: several chambers become partition walls in the print tree and nodes in the air network.
4. **Print splitting** (README 6.4). Cut the print solid into parts that fit the X2D (`catalog/printers/bambu-lab-x2d.json`: 256 x 256 x 260 mm) with alignment pins and holes. Report part count, mass, and print time per part, and export a 3MF with one object per part. Suggested split: the engine chooses the cut planes and the alignment features in the print tree, and the page applies them with manifold-3d. Today `print_split_note` in `seatlock-sim` only warns.
5. **Full 3D (`fdtd`) for builder cabinets with openings** (README 5.5). This is the hardest item. If it stalls, finish 1 to 4 and leave 5 well documented.
   - Build the voxel mask from the shape's `contains_air`. Today the page sends a rectangular inner size from `web/src/App.tsx`, and the button is disabled for every shape except `box`.
   - Put the source at the mount. Each duct to outside is an opening into an exterior air block with absorbing faces, and `H_k` comes from the flow through it. The working code path is `helmholtz_neck` in `engine/crates/seatlock-fdtd/src/lib.rs`; generalize it.
   - Two-resolution uncertainty (10 mm and 5 mm), progress messages from the worker, and cancel (cancel works now by ending the worker).
   - CHK-FDTD-4: 5 mm against `waveguide` for the CHK-BOX-3 cabinet as the builder makes it. Tuning within 3 %, SPL within 1.5 dB below 300 Hz.
   - Known failure, ignored on purpose: `exterior_block_doubling_moves_resonance_and_off_resonance_values_under_one_percent` in `engine/crates/seatlock-fdtd/tests/checks.rs`. The lossless port resonance rings longer than the 24000-step cap, so values near it do not converge. Do not loosen the test. Either validate a decay window against the closed form, or leave it until losses arrive at Level 3. Record the choice in STATUS.
   - Record measured run times against the budgets in README 5.5.

**Assignment 12: Level 2 M4** (`tasks/level-2.md` M4): the optimizer (README 11.3), cost beside every score, the cheap-against-reference comparison (README 9.2), the side-by-side build methods (README 6.2), and CHK-E2E-2 with `two-way-vented`. Before the optimizer depends on it, ask the owner STATUS question 1 (quality target). Question 2 (playback hardware) is also due during Level 2. One question at a time, with a recommendation.

**Then Level 3, and Fable.** The owner is holding a limited allowance of Claude Fable 5.2 for the hardest physics and asked to be told when the time comes. When Level 2 is complete, tell the owner it is time, and post a brief here for that session before it starts: `exterior` radiation by reciprocity (README 5.6), the DML panel and exciter (5.9, CHK-PLATE-1), the `simple` room (5.10, CHK-ROOM-1), losses in the 3D model, and the two 3D wave views (Level 3 track D). If item 5 above stalls badly, the owner may choose to spend Fable on it instead; offer that, do not decide it.

**The owner and the laptop**

- The owner is not a programmer (README 1.1). Report result first, in plain language, without file names.
- Laptop (Linux Mint): the repository is cloned, and Rust, build-essential, and Node.js 22.23.4 are installed. The last instruction was to run `bash start.sh` in the clone; the result has not been reported. If the owner pastes an error, fix `start.sh`. When it runs, mark Level 0 complete in STATUS.

**Pitfalls in the cloud workspace**

- Rust is the installed stable (1.97.0), not the pinned 1.99.1, because rustup cannot download. Set `RUSTUP_TOOLCHAIN=stable` for direct `cargo` calls.
- `tools/cloud-check.sh` builds the wasm32 standard library from source and links the preinstalled Chromium under the build number Playwright expects. Use it instead of `./check.sh`.
- The engine must not call `Instant::now` in WebAssembly; it panics there.
- CHK-E2E-1's stored values (`web/e2e/chk-e2e-1.json`) are regression values only. Change them only with a reason recorded in STATUS.
- A session hook may refuse to stop while the tree has uncommitted changes. Commit unfinished work to a branch (as `claude/shape-contract-wip` was) rather than leaving it uncommitted or pushing a failing `main`.

**Next step:** Assignment 11, item 1.

## Resolved

- 2026-11-11: Assignment 12 (Level 2 M4: cost beside score, reference build, build methods, optimizer, CHK-E2E-2) done on `main` by Claude; Level 2 complete.
- 2026-11-10: Assignment 11 item 5 (full 3D for builder cabinets with ports, CHK-FDTD-4, the exterior-block rule) done on `main` by Claude; Level 2 M3 complete.
- 2026-11-10: Assignment 11 items 1 to 4 (shapes, recesses and bolt holes, ports and chambers, print splitting) done on `main` by Claude; item 5 handed to the next lead session. Level 0 closed by the owner's laptop run.
- 2026-11-10: Cursor went inactive; Claude built directly, finished Assignment 10, added `sheet_box`, and handed off to the next lead session.
- 2026-11-10: Assignment 10 item 1 (shelf fix) merged into `main` by Claude; Cursor went inactive, and Claude continues the rest directly.
- 2026-11-10: Assignment 9 (review fixes and part of Level 2 M3) merged into `main` by Claude; the shape contract, the shelf fix, and the rest of M3 moved to Assignment 10.
- 2026-11-10: Assignment 8 (Level 2 M2) merged into `main` by Claude; five review fixes moved to Assignment 9.
- 2026-11-10: Assignment 7 (sourced drivers and Level 2 M1) merged into `main` by Claude.
- 2026-11-10: Assignment 6 (correction rework and Level 1 M4) merged into `main` by Claude.
- 2026-11-10: Assignment 5 (M2 fixes and Level 1 M3) merged into `main` by Claude; the correction rework moved to Assignment 6.
- 2026-11-10: Assignment 4 (Level 1 M2) merged into `main` by Claude; three fixes moved to Assignment 5.
- 2026-11-10: Assignment 3 (Level 1 M1) merged into `main` by Claude.
- 2026-11-10: Assignment 2 (Level 0 fixes) merged into `main` by Claude.
- 2026-11-10: Assignment 1 (Level 0 scaffold) merged into `main` by Claude; open items moved to Assignment 2.
