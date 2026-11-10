# Task: Level 1, one driver in a sealed box, end to end

Read README.md, STATUS.md, and COMS.md first. README section 12 lists Level 1; this file says how to build it and in what order.

Level 1 is done in four milestones. **Do one milestone per assignment.** Each ends with `./check.sh` passing and a handoff in `COMS.md`. Then wait for Claude's review before starting the next one. Each milestone leaves every track connected, per the level rule.

Rules for all milestones:

- Every check id named here becomes a test with that id in its name, using the numbers in README section 13. Do not loosen a tolerance. If a check fails and you believe the check is wrong, post a question in `COMS.md` with your derivation and stop work on that check.
- Physics and DSP live in the engine. The web side only edits the design and draws results.
- Pure-Rust crates only. Allowed without asking: `num-complex`, `rustfft`, `serde`, `serde_json`. Ask in `COMS.md` before adding any other engine dependency.
- Keep `STATUS.md` history: add lines, never rewrite the file.

## M1. Contracts, the design model, and the thin chain

Goal: the engine's `simulate` exists and answers for one cone driver in a sealed box, the extension-point contracts are in place, and the web app draws the results.

1. **Lossless design model.** Every design struct keeps unknown fields: add `#[serde(flatten)] extra: serde_json::Map<String, Value>` and turn on serde_json's `preserve_order` feature. `design.check` builds its canonical output from the typed model, not by echoing the input. The canonical form is serde_json's pretty printing with 2-space indentation and a trailing newline. Store `designs/examples/sealed-test.seatlock.json` in that form. Test: the file parses and re-serializes byte for byte; a file with an unknown top-level field and an unknown field inside a radiator also round-trips.
2. **Issues.** `Issue` gains `severity: "error" | "warning"`. Codes are lowercase snake case, for example `unsupported_kind` and `missing_field`. Remove the `ERR_`/`WARN_` prefixes.
3. **Registries (README section 3.3).** Replace the fixed `KindRegistry` sets.
   - One Rust trait per family in use at Level 1: `OuterShape`, `Radiator`, `EnclosureModel`, `RadiationModel`, `RoomModel`, `DspBlock`.
   - A `Registry` holds, per family, a map from kind string to a constructor that parses the raw JSON fields into a boxed member.
   - Each member registers itself with one function call, for example `cone::register(&mut registry)`. `Registry::builtin()` calls those functions, and nothing else knows the list of kinds.
   - Kind lookup goes by the part's position in the design schema: `speaker.enclosure.outer`, `speaker.radiators[i]`, `dsp.whole_speaker[j]`, `dsp.per_radiator[i].blocks[j]`, `speaker.room`. Do not match path substrings.
   - An unknown kind becomes an `Unsupported { kind, raw }` part. It is saved unchanged, left out of results, and reported with an `unsupported_kind` warning naming the field.
   - Change the workspace `members` to `["crates/*"]`, so that a new crate needs no edit outside itself.
4. **CHK-EXT-1.** Add a crate `engine/crates/seatlock-ext-example`, used only by tests. It registers a new outer shape (`test_cylinder`), a new radiator (`test_constant_source`, a fixed volume velocity per volt), and a new DSP block (`test_half_gain`). A test in that crate runs `simulate` on a design that uses all three and checks that the results reflect them. Nothing outside that crate changes.
5. **CHK-EXT-2.** A design with an unknown radiator kind next to a known cone. It opens, saves unchanged, `simulate` returns results for the cone, and there is one warning naming the unknown part.
6. **Catalog in the engine.** A `build.rs` in the crate that owns the catalog embeds every JSON file under `catalog/` and `targets/`. Adding a file needs no code change. Add `catalog/electronics/test-amp-a.json` (1 channel, 50 W into 4 ohm, synthetic) and `catalog/materials.json` with `pla` and `petg`. Their densities are 1240 and 1270 kg/m^3, marked as typical values to confirm against a filament data sheet. Their prices are `null` until the owner answers question 3 in STATUS. Cost shows "price unknown" when a price is null; never invent a price.
7. **Physics, `seatlock-acoustics`.**
   - `cone` radiator: README section 5.1.
   - `lumped` sealed box: section 5.3. Net volume is the gross interior volume minus each driver's `mounting.displacement_m3`. That is an optional catalog field copied from a data sheet; when it is missing, use 0 and warn.
   - `point` radiation with `space` set to `full`, `half`, or `step`: section 5.6.
   - The `free` room.
   - Limits and `max_spl(f)`: section 5.7.
   - Checks: CHK-DRV-1, CHK-DRV-2, CHK-BOX-1, CHK-BOX-2.
8. **DSP, `seatlock-dsp`.**
   - RBJ biquads (all seven types), gain, integer-sample delay, and polarity: README section 7.
   - One implementation gives both the complex frequency response and the sample-by-sample process function.
   - Checks: CHK-DSP-1, CHK-DSP-3.
9. **`simulate` (README section 3.2).** It returns per-radiator and summed complex pressure, impedance, cone travel, limits, `max_spl`, warnings, and a fidelity label on every curve (section 5.8). The DSP chain's response multiplies each radiator's response. Expose it as the engine method `simulate`. Large arrays may be JSON at Level 1; measure the reply time for sealed-test and record it in STATUS.
10. **Web.**
    - A form edits every Level 1 field of README section 4.1, in plain-language labels with units (section 14).
    - Every edit raises a revision number. The worker reply carries the revision, and stale replies are dropped.
    - Graphs: response in dB SPL, impedance in ohm, and cone travel in mm, each against frequency on a log axis from 20 Hz to 20 kHz. Each graph shows its fidelity label. Use a small charting library or plain SVG; record the choice in STATUS.
    - A Playwright test changes the box size and sees the response graph change.

Handoff: list each item as done, partly done, or not done, and give the tail of `./check.sh`.

## M2. Cabinet, 3D solver, and cost

1. **`box` outer shape (`seatlock-geom`).** Mounting surfaces `front`, `back`, `left`, `right`, `top`, `bottom` return a position and an outward direction for `[u, v]` (README section 4). Report interior volume, solid volume, and an inside test.
2. **Print solid.** Built with `manifold-3d` in a worker (README section 6.3): the outer shell minus the interior, minus the driver cut-out from the catalog `cutout_diameter_m`. Binary STL export in millimeters. Checks: CHK-GEO-1 and CHK-GEO-2. Run the CHK-GEO-1 comparison in the Playwright suite, since the solid exists only on the web side.
3. **3D view.** three.js shows the solid with the driver position. The view updates when the design changes.
4. **`fdtd` (`seatlock-fdtd`).** README section 5.5, closed box only: the voxel grid from the outer shape and a resistive source over the driver's aperture. Checks: CHK-FDTD-1 and CHK-FDTD-2 run natively. In the app, `Z_ab` from `fdtd` is drawn over the `lumped` curve and runs in its own worker with progress and cancel. Measure the time at 10 mm for sealed-test on the machine you use and record it.
5. **Cost (README section 9.2).** Drivers plus cabinet plus extras, times two for a stereo pair, plus electronics. Show "price unknown" for any null price. Check: CHK-COST-1.

## M3. Correction and score

1. **C1 and C2 (README section 8),** following all seven C1 rules. Every filter carries its plain-language reason. Checks: CHK-EQ-1, CHK-EQ-2, CHK-EQ-3. Correction output is a list of DSP blocks added to the design, and switching it off removes them.
2. **Metrics (README section 11.1):** `f6_hz`, `flatness_db`, `max_spl_db`, and cost on screen, each with its plain-language name.
3. **A one-variable sweep:** cost and `f6_hz` against box volume, drawn as a graph.

## M4. Listen mode and the end-to-end check

1. **The real-time kernel runs the design's DSP chain.** `seatlock-rt` depends on `seatlock-dsp` and uses its process code. The worker computes coefficients and sends numbers to the worklet, never text (README section 3). The kernel outputs one channel per radiator. No allocation in `process`. In the worklet, use the bound view directly when the block equals the capacity instead of calling `subarray` each block.
2. **The virtual speaker (README section 10).** The engine computes each radiator's impulse response from terminal voltage to pressure at the listening position. Then one `ConvolverNode` per radiator with `normalize = false`, and the results summed.
3. **Sources:** pink noise, a log sweep, a tone, and an audio file the user opens. Switching correction on and off crossfades. Volume maps to amplifier voltage. A cone-travel meter lights when a limit is exceeded.
4. **CHK-E2E-1 (README section 12).** Store the expected values. The headless browser test loads sealed-test, simulates, corrects, exports the STL and filter text, and checks score and cost.
5. **Run `./start.sh` and `./check.sh` on the owner's laptop if you are running there.** Otherwise say plainly that you are not.
