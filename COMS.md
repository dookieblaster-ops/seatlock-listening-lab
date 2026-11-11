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

**Then Level 3, and Fable.** When Level 2 is complete, tell the owner it is time, and post a brief here for that session first: `exterior` radiation (README 5.6), the DML panel and exciter (5.9, CHK-PLATE-1), the `simple` room (5.10, CHK-ROOM-1), losses in the 3D model, and the two 3D wave views (Level 3 track D).

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
