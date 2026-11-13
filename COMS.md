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

### 2026-11-13 — Claude — Handoff: start here. Walls for divided cabinets, sheet boxes, and tube caps

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Continued item 1 of the post below under the standing rule (no new owner go-ahead needed; it was queued work).

**Done:** README 5.11 ("Cabinets divided into chambers", "Stock cabinets", checks), 6.2, 6.4, 12, 15. `seatlock-acoustics`: `AcousticNode::Chamber { volume_m3, slot }` and `WaveguideResult.pressures` (pressure in each chamber per unit flow into a node, carried through ducts and parallel branches like the openings); `solve_with_walls(node, f, lossy, walls)`; `network::lumped_network_with_walls` and `NetworkSample.pressures`. `chambers::waveguide_network` emits `Chamber` nodes (bit-identical with no walls). `seatlock-sim/src/walls.rs`: `chamber_walls` (chamber sides against the outer wall found by marching outward through `contains_cabinet`, at most 1.5 walls), `PlacedWall.chamber`, `chamber_admittance`, `element_chambers`; `cabinet_walls` takes `sheet_box` (`sheet_layer`, `stock_figure`: a range at its middle, refuses without stiffness or density) and `tube` (end caps, square of the pipe's inside area). In `simulate`: walls go into each node solve (`wall_y` per bin per chamber), `node_pressures` keyed like `radiator_nodes`, wall sources use their own chamber's pressure; the old post-hoc shunt remains for one-chamber cabinets and for full-3D bins of the driver's chamber (`replaced_3d`). `web/e2e/generate.spec.ts`: own limit 15 minutes (it was 10 against an inner 14-minute wait, and failed the baseline check under load).

**Last full check on this commit:** `tools/cloud-check.sh`: rustfmt, clippy, 206 engine tests, bindings, both wasm modules, type check, vitest, build, 37 of 37 browser tests, "All checks passed" (stable 1.97.0).

**Honest to say:** partitions are held still; MDF and plywood boxes stay rigid until their records get a sourced density (MDF) and stiffness and density (plywood); a 3 mm foam-board box is absurdly floppy (its walls hold 4.4 times the air's compliance), which the lab now shows rather than hides.

**Next, in order:** (1) source MDF and plywood stiffness, density, and if possible loss factor (Wood Handbook or a maker's sheet, opened in the session; README 9.1), then their sheet boxes vibrate with no code change; (2) braces: a `brace` solid splits a wall's span (README 6.2); (3) partitions as two-sided plates between two chambers' pressures; (4) real damping figures (tap test on a printed strip, DMA data for the TPU cores); (5) soft walls' modal limit (residual mass term or a faster eigen solver); (6) the air's load on the walls; (7) the queue of the posts below (real exciters, STEP import, CTA-2034 score, boundary elements).

**Pitfalls:** as below. Do not run a second cargo build beside `tools/cloud-check.sh`: the generator's browser test is close to its time limit under load. A worktree with its own `CARGO_TARGET_DIR` lets you code while the check runs, but only after its build steps finish.

### 2026-11-13 — Claude — Handoff: start here. Walls vibrate and are heard

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-13): go-ahead for item 1 of the post below, "so sand-filled, plaster-filled, and two-filament walls change the simulated sound, not just the cost."

**Done:** README 5.11 (new), 3.2, 5.5, 5.6, 6.2, 6.4, 9.1, 12, 13 (CHK-WALL-1, CHK-WALL-2), 14, 15, 16. `seatlock-acoustics/src/wall.rs` (`WallLayer`, `WallStack` with the Mead-Markus / RKU stiffness `D(k)`, `WallPanel`: supported plate modes per wall size, cached, complex stiffness per mode, `admittance`, `element_admittances`); `PlateModes::cell_integrals` in `plate.rs` (separable 1D Gauss integrals); `seatlock-sim/src/walls.rs` (`wall_layers` from `build.material` and `build.wall`, `cabinet_walls`: flat surfaces from `surface_outline`, span less one wall, one patch per wall carrying its net flow (3 x 3 patches cost five times as much for at most 0.2 dB), walls under a panel held still); `SimulateRequest.walls`; in `simulate` the walls are a shunt on `Z_ab` and every `H_k` after the full 3D step, and each loading driver gets one extra source per wall patch (`-Y_e * Z_ab * U_d`). `walls: true` in `compare::request`, the wasm full 3D call, and `web/src/engineWorker.ts`. Catalog: sand and plaster loss factors, plaster stiffness (derived), varioShore stiffness (derived), both TPU loss factors, each with its source in the record. Numbers in STATUS, "What has been verified".

**Also fixed:** the page's engine call dropped `listening_window`, `losses`, and `walls` (`seatlock-wasm`, `simulate`); `bessel_j1` failed above an argument of about 20 (now asymptotic from 12). CHK-E2E-1 and 2 stored values moved for that reason (STATUS).

**Last full check on this commit:** `tools/cloud-check.sh`: rustfmt, clippy, 201 engine tests, bindings, both wasm modules, type check, vitest, build, 37 of 37 browser tests, "All checks passed" (stable 1.97.0).

**Honest to say:** the model is a fair first-order one; the biggest unknowns are material figures (PLA's damping is assumed, the TPU and plaster stiffnesses are derived, sand is taken as loose). It already shows real trade-offs: in `sealed-test` plaster-filled walls ring least; a soft core or loose sand between thin skins softens the wall more than its damping helps.

**Next, in order:** (1) walls for cabinets divided into chambers (each wall's span between partitions and solid plastic; each chamber's walls on its own pressure), and for `sheet_box` and `tube` (MDF and plywood records need published E and loss factor first); (2) braces: a `brace` solid splits a wall's span (README 6.2 lists it, not built); (3) real damping figures: a tap test on a printed strip (the page could take a recorded decay), published DMA data for the TPU cores; (4) soft walls (sand between thin skins, soft cores) resolve modes only to a few hundred hertz at basis degree 16: a residual mass term above the limit, or a faster eigen solver for a higher degree; (5) the air's load on the walls (mass and radiation damping), which lowers and widens each wall resonance a little; (6) the queue of the posts below (real exciters, measured loss factors, STEP import, CTA-2034 score, boundary elements).

**Pitfalls:** walls are off in the engine's default and in correction's own simulate call (`correction_plan`), on purpose. `wall_build_not_heard` now fires only when walls are not asked for or not modeled. A first call per wall size solves its modes; the cache holds 16 sizes. Do not edit Rust or catalog files while `tools/cloud-check.sh` runs (the catalog is embedded in the wasm build).

### 2026-11-12 — Claude — Handoff: start here. Novel materials, layered panels, wall builds

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-12): "Include all novel materials that have promise for a cheap HiFi grade speaker"; then "Do all 3 and add modern foaming 3D filament. Update readme and handoff package." The three were: source panel and damping materials, extend the panel model to direction-dependent and sandwich panels, and add wall materials with costs.

**Done:** `seatlock-acoustics/src/laminate.rs` (layers, classical laminate theory, shear through the thickness, assumed loss factor and Poisson's ratio); `Plate` now holds `layup: Stiffness` with `Plate::isotropic`; `dml.rs` reads `panel.layers`; catalog `panels/` (11 real), `damping/` (3), `fills/` (2), four foaming filaments in `materials.json`; `catalog::fill`, `catalog::damping`; cost: a line per panel layer, `build.wall` (`wall_build` in `cost.rs`), `name_of` prefers a record's `name`; warning `wall_build_not_heard`; wasm `catalog.list`; page `web/src/MaterialControls.tsx` (print material, wall build, panel layers with Sandwich and Plywood presets) and a damping-material choice for imported parts. README 3.1, 5.9, 6.2, 9.1, 12, 13 (CHK-PLATE-2), 14, 15, 16.

**Next, in order:** (1) a wall-vibration model so filled and two-material walls are heard (plate modes of each wall from the same Rayleigh-Ritz solver with a layered wall, driven by the inside pressure, radiating through the baffle model; constrained-layer damping of a soft core needs the core's shear loss, the RKU or Ross-Kerwin-Ungar model); (2) real exciters (Dayton Audio DAEX series data sheets: Re, Le, Bl, moving mass, suspension, travel) to replace `test-exciter-a`; (3) measured loss factors: a tap test on a printed strip (the page could take a recorded decay); (4) stock sheets for imported parts and STEP import (post below); (5) the queue of the posts below.

### 2026-11-12 — Claude — Handoff: start here. Imported speakers (3MF and STL), materials per part, damping

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-12): import complete speakers with internal baffles and damping, a material per part, drivers in any orientation; 3MF and STL now, STEP later.

**Done:** outer shape `mesh` (`seatlock-geom/src/mesh.rs`: parts, column grid, points as surfaces); `OuterShape::part_materials` and `damping_at`; per-part pricing in `cost.rs`; damping (Rayleigh) in `seatlock_fdtd::grid` via `Options.damping`, fed by `voxels::cabinet_grid`; page: `web/src/importMesh.ts` (STL, 3MF with units, components, transforms), `ImportedCabinet.tsx` (parts table, quarter turns, place by click, tilt and turn), click picking in `CabinetView.tsx`, op `mesh` in `printSolid.ts` and `openscad.ts`. README 3.3, 5.5, 6.2, 14, 15.

**Next, in order:** (1) a real damping material in `catalog/damping/` from a source with a published flow resistivity (polyester fibre, mineral wool), and a frequency-dependent model when such figures exist; (2) damping in the detailed model as a lossy line where a part fills a duct; (3) STEP import (OpenCascade in WebAssembly in a worker, tessellate to the same parts); (4) stock-sheet parts in an imported assembly (cut list); (5) port size and length editable on the page; print splitting per imported part; (6) the queue of the posts below (coupled solve for unlike woofers sharing a chamber, CTA-2034 score, off-axis files, larger woofers, boundary elements).

### 2026-11-12 — Claude — Handoff: start here. Each driver its own chamber's load; two woofers and three-ways in the generator

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-12): "continue on the project" (declined my suggested reordering; the standing queue holds).

**Done:** `AirNetwork.driver_nodes`; in `simulate` the network is solved from each chamber a driver pushes into (`other_nodes`, `radiator_nodes`), and drivers sharing a chamber get `Z_ab` times their count (`sharing_counts`), with label lines. Generator: `Family.woofers`, `VARIABLES = 15` (x[12] midrange, x[13] lower crossover, x[14] midrange level), `CHOICES`, four new families (indices 24 to 27). Tests `seatlock-sim/tests/chamber_loads.rs`; `tests/generate.rs` counts sources as `ways + woofers - 1`.

**Next, in order:** (1) a woofer on the back with another in front, and two woofers in a coupled solve when their settings differ; (2) the CTA-2034 curves and a preference rating (queue below); (3) the off-axis and impedance files from the Dayton packages (post below); (4) larger budget woofers (6.5 to 8 in.) with mounting sizes, since 95 dB to 50 Hz needs cone area; (5) the boundary-element model.

### 2026-11-12 — Claude — Handoff: start here. Makers' measured curves are in; then the queue below

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-12): keep 2.1 mm for the DSA175-8; downloads from respected HiFi community and industry sources are approved.

**Done:** `catalog/drivers/responses/<id>.json` for 11 Dayton drivers (on-axis FRD, every second point) and `response_file` in their records; `catalog::all` reads a folder's own files only; clearer curve label in `seatlock-acoustics/src/response.rs`; tests `seatlock-sim/tests/responses.rs`; README 5.6 and 9.1.

**How the files were fetched:** the cloud browser finds the "Graphs/FRD/ZMA Data File" link on each Parts Express product page; the linked built-in browser, on a parts-express.com tab, can `fetch` `/pedocs/...` zips from JavaScript and unzip them in the page (product pages themselves return a bot page). WebFetch and the workspace shell cannot reach these sites.

**Next, in order:** (1) the off-axis files (15, 30, 45 degrees, same packages) for the listening window and the CTA-2034 curves; the impedance files to check each driver's parameters; (2) more independent data (post below); (3) the queue of the post two below.

### 2026-11-12 — Claude — Handoff: start here. Independent driver measurements; then the queue below

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner (2026-11-12): yes to independent measurements beside the makers' data, labeled by source. The queue of the post below still holds, with its go-ahead.

**Done:** README 9.1 rule and glossary; `seatlock_acoustics::{with_independent, breakup}` (the cone radiator uses the merged record; the note names both values); `breakup_warnings` in `seatlock-sim/src/lib.rs` (code `measured_breakup`); the generator caps a two-way's crossover at half the woofer's measured break-up; new driver `catalog/drivers/dayton-dsa175-8.json` with a Voice Coil Test Bench entry; tests `seatlock-sim/tests/independent.rs`. Last full check on this commit: rustfmt, clippy, 169 engine tests, bindings, both wasm modules, type check, vitest, build, 34 of 34 browser tests, "All checks passed" (stable 1.97.0).

**Next, in order**

1. **More independent data.** Voice Coil Test Bench pages on audioxpress.com open in the cloud browser (WebFetch is refused there); search "Test Bench Dayton Audio <model>". Copy only printed numbers (T/S table text when it is text, Klippel XBl and XC, break-up peaks stated in the prose); the T/S tables are often images, so read them only when the text gives them. HiFiCompass (hificompass.com) opened once in the cloud browser and then timed out; its curves are images. Erin's Audio Corner has a drive-unit section (erinsaudiocorner.com/driveunits). Makers' FRD/ZMA data files (daytonaudio.com product pages, "Graphs/FRD/ZMA Data File") are measured curves: downloading one needs the owner's yes (cloud-browser rule); with them, README 5.6's response-file blend can be built.
2. The queue of the post below: any number of sound sources in the generator (the DSA175-8 is the larger woofer it asked for), the CTA-2034 score, the boundary-element model.

**Next step:** queue item 1 of the post below; item 1 here alongside when a source opens.

### 2026-11-12 — Claude — Handoff: start here. Two-ways in the generator; the plan from the industry research

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3. The owner (2026-11-12): "The software should work with any configuration ... do more research on how the simulator should be built because you're coming up with too many limitations ... do it." This is a standing go-ahead for the queue below.

**Done:** research (STATUS "What has been verified", README 2.6 "How the industry builds this"); two-way families in `seatlock-sim/src/generate.rs` (`Family.ways`, `tweeters()`, `VARIABLES = 12`: x[9] tweeter, x[10] crossover, x[11] tweeter level); `mount_features` in `seatlock-sim/src/lib.rs` leaves screw holes out when no bolt circle is published; DC28F-8 mounting and price (unconfirmed, `via`). Last full check on this commit: rustfmt, clippy, 166 engine tests, bindings, both wasm modules, type check, vitest, build, 34 of 34 browser tests, "All checks passed" (stable 1.97.0).

**Queue, in order (go-ahead given)**

1. **Any number of sound sources in the generator.** Make `build` take a list of drivers: three-ways (a midrange needs its own sealed chamber: a box chamber behind it, as `two-chamber` shows), two woofers, a woofer on the back plus one in front. Crossovers per neighbouring pair. Add larger budget woofers (6.5 and 8 in) to the catalog with sizes; 95 dB clean needs cone area. Open each part's own page; from this workspace parts-express.com data sheets were blocked, daytonaudio.com product pages appear in search results and may open.
2. **The standard curves as the score** (README 2.6 item 3). The radiation contract's `transfer` already gives pressure at any point: compute the 70 CTA-2034 directions under `baffle` (use the shared geometry trick of the sound-power code so it stays fast), form listening window, early reflections, sound power, predicted in-room response, and the directivity indices; show them as one graph; then a preference rating. Take the rating's formula and constants from a source open in the session (Olive 2004, AES paper 6190, or the standard), not from memory (README 1.3). Then let correction fit the listening window and let the generator chase the rating. The target's thresholds are the owner's (ask once the numbers exist).
3. **A boundary-element model** (README 2.6 item 2): first the outside (rigid cabinet skin from each outer shape's mesh, the print tree gives one; sources as moving patches; Burton-Miller or CHIEF points for the irregular frequencies), validated on CHK-RAD-3's sphere and CHK-RAD-2's disc; one solve per frequency then gives all 70 directions. Then the inside as a second subdomain joined at the openings. Dense complex solves with `nalgebra` are fine to a few thousand elements; keep it in a worker.
4. Generator speed; the page's fields for cabinets that are not boxes; the rest of the previous post's queue.

**Pitfalls:** as in the post below. The generator's browser test now allows 14 minutes.

**Next step:** queue item 1.

### 2026-11-12 — Claude — Handoff: start here. The lab generates cabinets; curved shape, wider score, losses

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3 in progress. The owner stated the product's intent on 2026-11-12 (README 2.6) and approved four items; all four were worked. No helper is active.

**First steps**

1. Fetch every branch (standing rule 1); nothing waits for review.
2. Run `tools/cloud-check.sh` (about 17 minutes now: the generator's browser test alone takes 5). Last run on this commit: rustfmt, clippy, 166 engine tests, bindings, both wasm modules, type check, vitest, build, 34 of 34 browser tests, "All checks passed" (stable 1.97.0).
3. README 1.1: the owner gives a go-ahead before new items start. STATUS question 10 was put to the owner at the end of this session.

**Done this session** (numbers in STATUS, "What has been verified")

- README 2.6, 6.1, 11.4, 12: generating is a goal.
- Generator: `seatlock-sim/src/generate.rs` (`families`, `generate_family`, `build`), wasm `generate.families` and `generate.family`, page `web/src/Generate.tsx` (reuses `runJob`, `ScoreCells`, `CostPlot`, `rank` exported from `CostQuality.tsx`), tests `seatlock-sim/tests/generate.rs`, `web/e2e/generate.spec.ts`.
- `revolved`: `seatlock-geom/src/revolved.rs`; CSG op `revolve` in `web/src/printSolid.ts` and `openscad.ts`; example `egg`.
- Listening window: `SimulateRequest.listening_window`, results `listening_window_db`, `axis_flatness_db`, `window_flatness_db`; `flatness_db` is the worse. `seatlock-sim/tests/window.rs`.
- Losses: `seatlock_acoustics::solve_lossy` and `WALL_LOSS`; `grid::Options::loss_per_s`, `RunningSpectra::decaying`, `Fit::extended_decaying`; `SimulateRequest.losses`; `WALL_DIFFUSE` and the wall count in `full_3d_enclosure`. Tests `seatlock-fdtd/tests/losses.rs`, `seatlock-sim/tests/losses.rs`, CHK-WG-4 in `seatlock-acoustics/tests/checks.rs`.
- `Domain::around_smooth` and `Cut` in `seatlock-fdtd/src/exterior.rs`: a cut-cell grid, measured, not used by the simulator.

**What is honest to say:** the generator works end to end, but nothing it proposes is close to the target; the curved shape is one family (`revolved`), not free forms; the 3D wall loss is an average; the curve-following grid did not pay off.

**Queue (owner's go-ahead needed)**

1. **Two-ways in the generator.** The three catalog tweeters print no `cutout_diameter_m` or `outer_diameter_m`, so `parts()` leaves them out. Open each maker's sheet (README 9.1) and add the sizes; then a second radiator on its own mount (a sealed-back tweeter needs no chamber), a crossover (`optimize.rs` has LR4 and LR2 and the frequency search), and families for it. A larger budget woofer with mounting sizes would also help reach 95 dB.
2. **Generator speed.** 323 s in the cloud browser for 17 families of 16. Each scoring is correction plus simulation, about 1 s natively, more in wasm. Options: fewer frequencies while searching, score without correction first and correct only the best, drop families that cannot fit any driver early.
3. **The page for cabinets that are not boxes.** The Speaker panel edits width, height, and depth only; a generated egg, prism, or leaning front opens and simulates, but its shape fields cannot be edited on the page. A generic editor from the outer shape's own fields is the fix.
4. **Losses, further.** Damping fill (a flow-resistance term in the 3D grid's velocity update and in the detailed model; needs a material's flow resistivity from a source). A per-wall loss in the 3D grid in place of the average. Replace shaping's assumed uniform loss by `loss_per_s` (shaping runs in `adjoint.rs`, which has its own decay weight).
5. **The shadow error on curved bodies.** `around_smooth` removed a quarter of it. Test the dispersion idea: the same sphere at 20 cells per wavelength with stair-steps, and the error against path length around the body. If it is dispersion, a fourth-order stencil or a corrected sound speed is the fix, not cut cells.
6. **Shaping's fragility.** Its result changes with the last bit of a Fourier sum (STATUS). A robust optimizer would not; try several starts and keep the best, or smooth the finishing stage's accept rule.
7. `loft`, a body turned about the vertical axis, and the earlier queue (post below).

**Pitfalls:** as before. New: do not edit Rust files while `tools/cloud-check.sh` runs (its wasm build comes after the tests and would pick the edits up). `cargo fmt` needs `RUSTUP_TOOLCHAIN=stable`. `grid::dft` keeps separate arithmetic for zero decay on purpose (see queue item 6); do not "simplify" it. A design 4000 m wide runs the simulator out of memory.

**Next step:** read the owner's answer to STATUS question 10; then queue item 1.

### 2026-11-12 — Claude — Handoff: start here. Inside-shaping (brief items 1 to 6) and the OpenSCAD export are in

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3 in progress. The session began on Claude Fable 5.2 (brief items 1 and 2, most of 3 and 4) and the owner switched it to Claude Opus 5.5 partway; whether any Fable allowance is left is the owner's to say. No helper is active.

**First steps**

1. Fetch every branch (standing rule 1); nothing waits for review.
2. Run `tools/cloud-check.sh` (about 11 minutes). Last run on this commit: rustfmt, clippy, 150 engine tests, bindings, both wasm modules, type check, vitest, build, 31 of 31 browser tests, "All checks passed" (stable 1.97.0). `cargo test -p seatlock-sim --test shape -- --include-ignored` adds the 10 mm shaping run (4 minutes); with `WRITE_EXAMPLE=1` it rewrites `designs/examples/shaped-box.seatlock.json`.
3. README 1.1: the owner gives a go-ahead before new items start. The owner asked for the OpenSCAD export on 2026-11-11 and said "Go finish" for the shaping work.

**Done this session** (numbers in STATUS, "What has been verified")

- `seatlock-fdtd`: `grid::Options::relaxed`, `adjoint.rs` (`sensitivity`: `Z_ab`, `H_k`, and their derivatives for any faces; derivation in its module text).
- `seatlock-sim/src/shape.rs`: `Shaper` (objective and gradient per cell), `shape_inside` (density stage, finishing stage, `printable`), `apply` (writes the `voxels` solid, id `shaped`). `voxels::CabinetGrid::chamber_air`, `cabinet_grid_of`.
- `seatlock-sim/src/solids.rs`: the `voxels` solid (parse, encode, `contains`, `plastic_m3`, merged `boxes`); taken up by `chambers::Chamber::plastic_m3`, the single-chamber net volume in `simulate`, `voxels::cabinet_grid`, and `geometry_report_with` (print tree cut to the air, plastic volume, 5 mm voxel volume, pins).
- wasm method `shape.inside` (returns `{ result, design }`); page `web/src/ShapeInside.tsx`; `web/e2e/shape.spec.ts`.
- `web/src/openscad.ts` (`printToScad`), `hullsOf` in `printSolid.ts`, the "Export OpenSCAD file" button; four browser tests in `geometry.spec.ts` (set `SCAD_OUT=/tmp/scad` to keep the files for a render in OpenSCAD).
- README 3.3, 5.5 ("Shaping the inside"), 6.2 (`voxels`), 6.4, 14.

**What is honest to say about the shaping result:** it works end to end and the gradient is exact, but the gain is small on the one cabinet measured (0.48 to 0.38 dB). Three reasons, each a queue item: the assumed loss smears resonances to 32 Hz wide, so a plain sealed box has little to fix; thresholding part-air cells loses about half the density stage's gain; and only a sealed box has been tried since the optimizer was reworked.

**Queue (owner's go-ahead needed)**

1. **Make shaping earn more.** (a) Run `shape_inside` on `folded-line` and `two-way-vented` with the reworked optimizer and record what it does; `folded-line` at 20 mm did not finish in 10 minutes before (one more run per opening, 1.4 million cells at 10 mm): give `Shaper` a region box and a smaller exterior block. (b) Lower `decay_per_s` (30 gives 0.68 dB to work with on the tall box, three times the run length). (c) A stronger finish: single-cell moves ranked by the gradient with a fresh run are greedy; a level-set or a continuation with a volume-preserving threshold may keep more of the density stage's gain. (d) An objective at the listener (through `baffle`), and a weight that includes the standing waves' decay, not only the steady level.
2. **Losses in the 3D model** (README 5.5), then replace the assumed uniform loss in shaping by the real one. Then the 3D wave views.
3. **Named settings in the OpenSCAD file** (width, height, wall, port sizes at the top, so a number can be changed in OpenSCAD). The print tree would need to carry field names or expressions; offered to the owner, not built. Also not done: opening the file in FreeCAD, and a faster render for cabinets with many round holes (`$fn` 96 everywhere; `two-way-vented` takes 97 s in OpenSCAD 2021.01).
4. **A registry for internal elements** (README 3.3): chambers, ducts, and solids are still read directly by `seatlock-sim`; `brace` is in the README's example and is not built (it now warns).
5. The earlier queue, unchanged: a smaller exterior block and WebGPU (fast 3D step 1's rest), the library of 3D-solved parts and the line optimizer (step 2), room mapping, the panel's open items, the rest of Level 3.

**Smaller open items from this session**

- "Shape the inside" on the page always uses the default band, loss, and 15 % plastic; no region box is offered. Browser run time is not measured on the owner's laptop (20 mm on `sealed-test` finishes inside the browser test's limit in the cloud).
- OpenSCAD 2021.01 renders `shaped-box` with "may not be a valid 2-manifold" (placed plastic only; the shell is fine, and the lab's own STL passes CHK-GEO-2). Not explained: look at where two wall-gap slabs meet at a cabinet corner, and at `Voxels::boxes` output cut by `blocks - (blocks - air)`. `Voxels::plastic_m3` reads 0.55 % low on that example (6 points per axis against a wall 2 mm into a cell); more points on cells that straddle a wall would fix it, at a cost in every simulation.
- The shaped plastic is not drawn differently in the 3D view; it shows as part of the solid.
- `voxels` in a cabinet that is not printed (sheet box, tube) is simulated but not in the cost or a print file (warned).
- A `voxels` cell's plastic inside a duct's bore is cut by the bore in print but still counted in the chamber's volume; the shaper never places plastic there.

**Owner questions open:** 2 (playback hardware), 6 (panel material, wall distance, in-room tilt), 8 (depth-scanning phone).

**Pitfalls:** as before (`RUSTUP_TOOLCHAIN=stable`, `cargo fmt --all` from `engine/`, `tools/cloud-check.sh`, `npm run build` before one browser spec). New: `cargo fmt` without `RUSTUP_TOOLCHAIN=stable` tries to download the pinned toolchain and fails without formatting.

**Next step:** read the owner's answer; then queue item 1(a).

### 2026-11-11 — Claude — Brief for the Claude Fable 5.2 session: the 3D model shapes the inside (adjoint optimization)

- **To:** next lead session, which the owner will run on Claude Fable 5.2 with the last 23 % of that allowance (owner, 2026-11-11). Spend it on this brief, not on reading or on other queue items.
- **Status:** handoff
- **Context:** `main`. Level 3. Owner request: "iterate the 3D designs quickly to find the best form"; research found three steps, step 1 (faster runs) is in; this is step 3. No helper is active.

**Start (15 minutes at most)**

1. Fetch every branch (standing rule 1). Start `tools/cloud-check.sh` in the background (about 9 minutes; last run on this commit: rustfmt, clippy, 143 engine tests, bindings, both wasm modules, type check, vitest, build, 25 of 25 browser tests, "All checks passed", stable 1.97.0; the on-request `folded.rs` 3D check also passed, 72 s). Do not edit Rust files while it compiles.
2. Read only: README 5.5 (all of it, including "Stop early by harmonic inversion"), 6.2 "Inside", 11.3; this post. Code: `engine/crates/seatlock-fdtd/src/grid.rs` (`Grid`, `run`: per-axis face masks `open` are already `f32` 0/1, `specials` for source and absorbing cells, running Fourier sums), `seatlock-fdtd/src/extrapolate.rs`, `seatlock-sim/src/voxels.rs` (`cabinet_grid` builds the `Grid` from a design), `full_3d_enclosure` and `driver_impedance_scale` in `seatlock-sim/src/lib.rs`.
3. The owner is not a programmer: report in plain language, result first (README 1.1).

**Goal.** One forward and one adjoint 3D run give the gradient of a sound objective with respect to every face of a design region inside the cabinet. An optimizer uses it to shape that region (air or plastic, cell by cell), and the result is a printable inside that measurably beats the plain cabinet in a fresh forward run. Reference: Bokhari, Berggren, Noreland, Wadbro, "Loudspeaker cabinet design by topology optimization", Scientific Reports 13:21248 (2023), doi 10.1038/s41598-023-46170-4 (frequency-domain FEM, adjoint gradients, MMA, density filter and projection). Ours is time-domain on the existing grid: the novel part.

**Order. Commit and push after each item; each must stand alone.**

1. **Relaxed faces.** In a design region (a box of cells given in `Options` or on `Grid`), each face's open fraction `a` in [0, 1] replaces the 0/1 mask in the velocity update (`v -= kv * a * (p_ahead - p_behind)`). Everything outside the region is unchanged, and `a` = 0 or 1 must reproduce today's results exactly (assert bit-identical on CHK-FDTD-1's box).
2. **Adjoint gradient, validated before anything else.** Objective `J` from the frequency-domain outputs the run already makes (`Z_ab`, `H_k` at a few frequencies, at most about 16). Derive the discrete adjoint of the leapfrog update with the folded boundary admittance and the resistive source (README 5.5); the adjoint run is the same scheme driven at the aperture probe and the opening planes by `dJ/d(signal)`. Accumulate, over design faces only, Fourier sums of the forward pressure difference across each face and of the adjoint face velocity at the objective frequencies (memory: design faces times frequencies, complex). Then `dJ/da = Re sum over frequencies of (constants) * (forward) * (adjoint)`. **Check: on a small grid (20 mm cells, a box like CHK-FDTD-1, a design region of a few dozen faces), the gradient matches central finite differences of `J` within 2 % on at least ten faces, at `a` = 1, at `a` = 0, and at `a` = 0.5.** Record the derivation in STATUS. Run forward and adjoint to full decay for this check (`extrapolate: false`); decide and record whether the early stop is safe for gradients (the Fourier sums per face would need the same tail fit, or a full run).
3. **Objective tied to what the owner hears.** In `seatlock-sim`: `J` from the summed level against a target band (for example the sixth-octave deviation from flat between 150 Hz and 1 kHz, where cabinet standing waves live), through the driver and radiation models. `dJ/dZ_ab` and `dJ/dH_k` by the chain rule; the radiator's response to `Z_ab` is `A / (B + C * Z_ab)` (see `driver_impedance_scale`), so two evaluations give it without opening the radiator. Check the full chain by finite differences on one face as in item 2.
4. **Optimizer.** Density filter (radius about 1.5 cells), smoothed projection with increasing sharpness, a cap on the plastic added, and a simple, robust update (MMA, or projected gradient with a line search). Keep the plastic printable: no air pockets sealed off from the rest of the air, no features thinner than two cells. **Check: on `sealed-test` (or a taller box with a strong standing wave), the thresholded 0/1 design, run again forward from scratch, improves the objective by a stated amount, with the starting and final numbers in STATUS.**
5. **As a design.** A new internal element kind for `solids`, for example `{ "kind": "voxels", "min_m": [...], "dx_m": ..., "size": [nx, ny, nz], "cells": "<run-length encoded>" }`, registered as a member (README 3.3): it takes air from the quick and detailed models (volume), is solid in `voxels::cabinet_grid`, and prints as a union of merged boxes. Check CHK-GEO-1 and CHK-GEO-2 in the browser on an example that uses it. Raise `schema_version` only if an existing field changes meaning (README 3.3).
6. **On the page**, only if time remains: "Let the 3D model shape the inside" for a chosen chamber, in a worker with progress and cancel, showing before and after.

If time runs short, stop after item 2 or 3 with everything committed, and write a handoff that lets an ordinary session do 4 to 6 without re-deriving anything.

**Pitfalls.** Cloud workspace: `RUSTUP_TOOLCHAIN=stable`; `cargo fmt --all` from `engine/`; `tools/cloud-check.sh`, not `./check.sh`; no `Instant::now` in WebAssembly. The 3D runs take tens of seconds in test builds: keep gradient checks small, mark long ones `#[ignore]` with their last result in STATUS. Lossless resonances ring for a long time (README 5.5); losses are not built yet. Do not weaken any check (README 1.3).

**Next step:** item 1.

### 2026-11-11 — Claude — Handoff: start here. Inside shape done; shape optimizer waits for the owner's go-ahead

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3 in progress. The owner set the order this session: first "inside shape" (done), then possibly an optimizer that "iterates the 3D designs quickly to find the best form" (asked, not yet approved).

**First steps**

1. Fetch every branch (standing rule 1); nothing waits for review.
2. Run `tools/cloud-check.sh` (about 9 minutes). Last run on this commit: rustfmt, clippy, 141 engine tests, bindings, both wasm modules, type check, vitest, build, 25 of 25 browser tests, "All checks passed" (stable 1.97.0). One more engine test runs on request: `cargo test -p seatlock-sim --test folded -- --ignored` (full 3D of `folded-line`, about 8 minutes in the test build; it passed this session).
3. Read the owner's answers in the chat or STATUS questions 8 and 9 before choosing work. README 1.1: the owner gives a go-ahead before new items start.

**Done this session**

- Bent and tapered ducts everywhere: `ports::run` (straight runs, corners lengthened by half the section, slots bend in one plane, tapers in both shapes, hull op for a tapered slot), `ports::bores` returns one bore per run, print volumes along the center line, `PrintedPort.lines` keeps pins off every run.
- The detailed model for networks: `chambers::waveguide_network` (tree from the driver's chamber, taper orientation by direction, loops refused with fallback). Used for several chambers and for the single-chamber case.
- `compare::ENCLOSURE = "waveguide"`: the everyday model for scores, correction, optimizer, listening; the page's engine worker sends it too.
- Removed the wrong "Openings are not part of the sealed-box model" warning and the "sealed-box model" label line.
- Page: `InsideEditor` and `PathField` in `web/src/App.tsx` replace the chamber and duct JSON boxes (bracing is still JSON).
- Example `designs/examples/folded-line.seatlock.json`; tests `seatlock-sim/tests/folded.rs`, `web/e2e/inside.spec.ts`, and `folded-line` in the geometry spec's CHK-GEO list.

**Queue, in order (owner's go-ahead needed for 1 and 4)**

1. **Shape optimizer** (owner request; STATUS question 9). Extend `seatlock-sim/src/optimize.rs`: variables for line length (move corners), fold count (rewrite the path), taper (end size), chamber size, slot size, port; score every candidate with `waveguide` + `baffle` (milliseconds); check the best few with `fdtd`; show the shapes beside each other. The optimizer today scales boxes, sheet boxes, and tubes only (STATUS decisions 2026-11-11); folded lines need a generator that keeps the line inside the cabinet and printable (`ports::run` and `printed_ports` say why a candidate fails).
2. **3D cabinet speed for big mouths.** `folded-line` at 10 mm: 168 s natively, release. The exterior block reaches five opening widths (0.55 m for the 100 mm slot) and the lossless line rings to the 1 s cap. Options: losses (README 5.5, Level 3), the one-way boundary of `seatlock-fdtd/src/exterior.rs` on the cabinet solver's block (recheck the block rule in `seatlock-fdtd/tests/checks.rs`), or a block sized by the slot's area rather than its longest side (needs validation). The page runs 10 mm and then 5 mm; for this example 5 mm would be about 10 million cells and very slow in the browser: consider stopping at 10 mm when the cell count is high, and say so.
3. **Losses in the 3D model, then the 3D wave views** (unchanged from the Fable handoff below).
4. **Room mapping** (owner request; STATUS question 8): typed-in room size and positions first, a bass-only 3D room model and early reflections; phone scan import and microphone measurement later.
5. The panel speaker's open items and the rest of Level 3 (Fable handoff below): loops in the detailed model, fill, passive radiators, drivers on ducts, `revolved` and `loft`, FIR, C5 to C7, C10, undo and redo.

**Known limits of this session's work** (also in the labels): bends are straight in the detailed model; chambers are compliances (no standing waves) outside full 3D; the quick model leaves out a line's own air; round bends are counted along the center line (close, not exact); the geometry report's air volume for chambers is the gross chamber air, not the line's.

**Pitfalls:** as before. New: `cargo test` of `folded.rs` with `--ignored` takes about 8 minutes; the `InsideEditor` path box keeps half-typed text locally and publishes only a valid path.

**Next step:** read the owner's answers; then queue item 1 if approved, otherwise item 2.

### 2026-11-11 — Claude Fable 5.2 — Handoff: start here. Level 3 after the room, the outside in 3D, and the panel speaker

- **To:** next lead session (any Claude model)
- **Status:** handoff
- **Context:** `main`. Level 3 in progress. Fable brief items 1 to 3 done; item 4 not started. No helper is active.

**First steps**

1. Fetch every branch (standing rule 1); nothing waits for review.
2. Run `tools/cloud-check.sh` (about 8 minutes). Last run on this commit: rustfmt, clippy, 137 engine tests, bindings, both wasm modules, type check, vitest, build, 23 of 23 browser tests, "All checks passed" (stable 1.97.0).
3. Read README 5.5, 5.6 (the `exterior` "As built"), 5.9 (the panel "As built"), 5.10, and 12 (Level 3). Then this post's queue.

**Done in this session**

- Item 1, the `simple` room (README 5.10, CHK-ROOM-1): `seatlock-acoustics/src/room.rs`; room contract `images`, `needs_sound_power`, `in_room`, `notes` in `seatlock-core/src/registry.rs`; sound power and directivity index; a design with a room is scored on its in-room level; page: Room switch and "Loudness in the room" graph. Behind-the-surface sound under `baffle` (`baffle_shadow_pressure`).
- Item 2, `exterior` radiation (README 5.6, CHK-RAD-3, CHK-RAD-4): `seatlock-fdtd/src/exterior.rs` (scattered field, analytic incident wave, one-way outer layer), `seatlock-sim/src/exterior.rs` (radiation member, skin cells, 14-direction sound power); radiation contract `transfer` and `sound_power` with positioned `SourcePatch`es; page: "Run the outside in 3D" (`OutsideThreeD` in `web/src/App.tsx`, `outside.spec.ts`).
- Item 3, the `dml_panel` radiator (README 5.9, CHK-PLATE-1, CHK-E2E-3's `rear-panel` half): `seatlock-acoustics/src/plate.rs` (Rayleigh-Ritz modes), `dml.rs` (exciter, coil, cavity, surface elements, mode cache); radiator contract `surface`, `loads_box`, `notes`; `simulate` places elements with `mount_pose` and drops those off the outline; shared edge term for more than 32 sources on a surface under `baffle`; cost lines for exciter and panel; nothing cut for a source that names no driver; synthetic `catalog/exciters/test-exciter-a.json` and `catalog/panels/test-panel-material-a.json`; example `designs/examples/rear-panel.seatlock.json`; page: `PanelSource` fields, `panel.spec.ts`.

**Work queue, in order**

1. **Losses in the 3D model** (README 5.5, Level 3 track D): damping fill (flow resistance) and wall losses in `seatlock_fdtd::grid`. Afterwards resonance peak heights can be shown (today labeled as not modeled), and the ignored block-doubling test in `seatlock-fdtd/tests/checks.rs` may be revisited. Validate against a closed form (a lossy duct's `alpha` from README 5.4) before trusting.
2. **The two 3D wave views** the owner asked for (README 12 Level 3 track D): pressure inside the cabinet as a slow-motion pulse and as a standing-wave pattern at a chosen frequency (record snapshots from `grid::run`; draw slices in the three.js view), and a bass-only 3D wave model of the room below about 300 Hz with coarse cells, beside the `simple` room.
3. **Panel open items:** open backing (needs the panel as a thin body in the `exterior` grid, thinner than a cell); the air's load on the panel; a faster eigenvalue solver (Jacobi at degree 32 takes 1.4 s; a tridiagonal QR or Lanczos would allow a higher limit); the panel's frame in the print tree and cost; real exciters and panel materials with sources open (README 9.1).
4. **Rest of Level 3** (README 12): folded and tapered lines, coupled chambers, fill, passive radiators, drivers on ducts, `waveguide` for tree networks of chambers; inside-geometry editor; `revolved` and `loft`; FIR and linear-phase crossovers; C5, C6, C7, C10 and the target's tilt (today correction fits the direct sound even for room designs); Level 3 optimizer; undo and redo; stale-price warnings and alternative vendors; CHK-E2E-3's `folded-line` half.

**Smaller open items found this session**

- Stair-steps on curved cabinets read high in deep shadow under `exterior` (up to 3.4 dB at 10 mm); cut cells that follow the surface would fix it. It is in the fidelity label.
- `exterior` progress is per run, not over all runs; stale 3D results on the page are not grayed out (README 14).
- Listen mode does not include the room (README 10, Level 3 track H) and does not play limit behavior yet.
- The cabinet solver still uses the folded-admittance absorbing faces; the one-way layer of `exterior.rs` might let its exterior blocks shrink, which would help the 26 s preview time. Recheck the block rule if changed.
- Older items still open: the 20 L ported preview time, the vented self-check at 5.1 %, the "Openings are not part of the sealed-box model yet" warning on every ducted design, bent ports in print, the SAKRETE tube's wall.

**Owner questions:** 2 (playback hardware) and 6 (panel material, distance from the wall, in-room tilt) are open; 6 was put to the owner at the end of this session with a recommendation. Record answers in STATUS and the README.

**Pitfalls:** as in the posts below (stable Rust in the cloud, `tools/cloud-check.sh`, no prettier, `npm run build` before rerunning one browser spec). New: the `exterior` tests and the panel tests take tens of seconds each in the test build; panel mode solves are cached per process, so a timing measured second is not a cold one.

**Next step:** queue item 1.

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

- 2026-11-13: Walls for cabinets divided into chambers, foam-board sheet boxes, and tube end caps done on `main` by Claude.
- 2026-11-13: Wall vibration (sand-filled, plaster-filled, and two-filament walls heard) done on `main` by Claude.
- 2026-11-12: Novel materials (panel, damping, wall fill, foaming filament records), layered and direction-dependent panels, and priced wall builds done on `main` by Claude.
- 2026-11-12: Imported speakers (3MF and STL, materials per part, damping in the 3D model, drivers placed by clicking) done on `main` by Claude.
- 2026-11-12: Per-chamber driver loads, two woofers around a tweeter, and three-ways in the generator done on `main` by Claude.
- 2026-11-12: Makers' measured on-axis curves for 11 drivers done on `main` by Claude.
- 2026-11-12: Independent driver measurements (rule, engine, first driver DSA175-8 with Voice Coil Test Bench data) done on `main` by Claude.
- 2026-11-12: Two-way cabinets in the generator and the industry research done on `main` by Claude; any number of sources, the CTA-2034 score, and a boundary-element model queued with the owner's go-ahead.
- 2026-11-12: The owner's four items (spec around generating, the cabinet generator, a curved outer shape, a wider score with air losses) done on `main` by Claude; a curve-following 3D grid was tried and not adopted.
- 2026-11-12: Fast 3D step 3 (relaxed faces, gradient, objective, optimizer, the `voxels` solid, the page) and the OpenSCAD export done on `main` by Claude (Fable 5.2, then Opus 5.5); making shaping earn more is queued.
- 2026-11-11: Fast 3D step 1, the early stop by harmonic inversion, done on `main` by Claude (2 to 3 times faster for ported cabinets); the rest of step 1 and step 2 queued, step 3 briefed for Claude Fable 5.2.
- 2026-11-11: Inside shape (bent and tapered ducts in all models and in print, the detailed model for networks, the inside editor, `folded-line`) done on `main` by Claude.
- 2026-11-11: Fable brief items 1 to 3 (the `simple` room, `exterior` radiation, the `dml_panel` radiator) done on `main` by Claude Fable 5.2; item 4 (3D losses and wave views) handed on.
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
