# Seatlock Listening Lab

Seatlock Listening Lab is a speaker design simulator that runs in the browser: a DIY hi-fi playground. The user picks real, purchasable drivers, shapes a cabinet, both its outer form and its internal geometry, from stock materials, 3D-printed parts, or a mix, sets up a digital or passive crossover and digital correction, and the lab shows and plays what that speaker would do and what it would cost to build. It is a place to try unconventional ideas, such as novel cabinet shapes and rear-facing DML panels, before spending money on them. The lab does not only check shapes the user draws: it generates novel and complex cabinets itself, inside and outside, with different components in different positions and facing different ways, and models what each does to the sound (section 2.6).

The goal is hi-fi sound for the least money. Anyone can build a hi-fi system from expensive components; this lab is for doing it cheaply. Cheap off-the-shelf drivers and materials, digital processing, and printed geometry stand in for expensive drivers, large boxes, and passive crossover parts. Quality components and conventional materials stay in the catalog as a reference, so every cheap design can be compared with a conventional build of the same speaker. The simulator's job is to show which trades work before any money is spent.

**This README is the specification. It is written for the AI coding agents that build the program.** Read all of it before your first change in a session, then read `STATUS.md`, which records what exists today, what was decided along the way, and what to do next. If your context is too small for the whole file, always read sections 1 to 4 and 12, plus the sections for the part you are working on.

| Sections | Read when you work on |
| --- | --- |
| 1 Working here, 2 Product, 3 Architecture, 4 Conventions, 12 Build plan | anything |
| 5 Physics, 13 Checks | driver, box, waveguide, 3D solver, radiation, panel, or room code |
| 6 Cabinet builder | geometry, the 3D view, print export |
| 7 Crossover and DSP, 10 Listen mode | filters and real-time audio |
| 8 Correction and auto EQ | correction |
| 9 Catalog and cost | drivers, prices, materials |
| 11 Score, targets, optimizer | scoring and search |
| 14 Interface | screens and wording |

## 1. Working in this repository

### 1.1 The owner

The owner designs speakers and directs this project in plain language. The owner is not a programmer and does not read code. That shapes how you work:

- **Decide technical questions yourself.** Choose libraries, structure, algorithms, and fixes without asking. Record any decision a later session would need in `STATUS.md`, one line plus the reason.
- **Ask the owner only about the product**: what the lab should do, how good is good enough, what things cost, what hardware they own. Ask in plain language, one question at a time, and include your recommendation.
- **Report in plain language, briefly, result first.** Say what the owner can do now that they could not do before, what is still missing, and what they need to decide. Leave out code, file names, and jargon unless asked.
- **The owner runs one command**, `./start.sh`. Anything more complicated is yours to automate.
- **When the owner lists problems or changes, collect them and wait for a go-ahead before editing.** They often think of more items while describing the first ones, and starting early means redoing work. Summarize the list back, then wait.
- **Report only what you verified.** If you did not run it, say so.

### 1.2 Every session

1. Read this file, `STATUS.md`, and `COMS.md`. The newest handoff at the top of the active thread in `COMS.md` says what to do next (section 1.4).
2. Run `./check.sh`. In Claude's cloud workspace, run `tools/cloud-check.sh`, which takes the same steps there (`STATUS.md`, "How the work is done"). If it fails before you have changed anything, repairing it comes first.
3. Choose work using the level rule in section 12.
4. Work in small commits. Every commit leaves `./check.sh` passing.
5. Before you stop, update `STATUS.md`, post a handoff in `COMS.md` that the next session can start from without asking anyone, commit, and push.

You start each session with no memory of the last one. Whatever is not in the repository is lost, so `STATUS.md` and the newest handoff have to be true at the end of every session.

### 1.3 Rules that protect the simulator

A simulator is worth using only if its numbers can be trusted. These rules exist for that reason.

- **Do not invent data.** Driver parameters, prices, and material properties come from a source you have open in the same session (section 9). Your own memory of a part is not a source.
- **Do not weaken a check to make it pass.** A failing check from section 13 means the code is wrong until shown otherwise. If you conclude the check itself is wrong, prove it with an independent derivation, record the proof in `STATUS.md`, and only then change the check.
- **Label every result with the model that produced it**, and list what that model could not represent (section 5.8).
- **Keep physics out of the interface and the interface out of the physics** (section 3).
- **Keep this README true.** When the owner changes what the product is, or you find the specification wrong, change the README in the same commit as the code and note it in `STATUS.md`.

### 1.4 Sessions, handoffs, and helpers

Each chat is a separate session with no memory of the others. Sessions pass work to each other through `COMS.md`, not through chat the owner may not see. The owner starts a session by opening a new chat and sending: `Work on my GitHub repository dookieblaster-ops/seatlock-listening-lab. Attach it, then Check COMS.md.` The same paste lives in `docs/new-chat.md`. No project is needed: the session attaches the repository itself. A chat that already has it needs only **Check COMS.md** (then **Go** to continue the queue).

| Role | Who | Job |
| --- | --- | --- |
| Lead architect and engineer | A Claude session, or a Cursor cloud agent (from 2026-11-14) | Owns architecture, cross-track order, and changes to this specification, and writes the code. Ends every session with a handoff in `COMS.md`. Assigns work to helpers and reviews and merges their branches. Pushes finished lead work to `main`. |
| Helper | Another coding agent: a second Claude chat, or a Cursor agent on a branch | Implements the assignment the lead posted, on its own branch. Runs the checks, keeps commits small, and reports only verified results. Escalates gaps in the spec to the lead through `COMS.md`. Never merges into `main`. |

Only one lead session works on `main` at a time. When a task needs a stronger model than the one in use, the lead tells the owner and posts a brief in `COMS.md` for that session; `STATUS.md` records which work is held for it.

Message format, thread rules, and the live board are in `COMS.md`. Older threads move to `docs/coms-archive.md`. Product decisions for the owner still belong in `STATUS.md` (section 1.1), not in `COMS.md`.

## 2. The product

### 2.1 One design, one chain

Everything the lab shows comes from one design document passing through one chain:

```
music or test signal
  -> correction applied to the whole speaker
  -> digital crossover: one branch per radiator
  -> per-radiator correction, delay, gain, limiter
  -> amplifier channel, one per radiator
  -> radiator              a cone driver with its cabinet air, or another kind such as a DML panel
  -> radiation             every moving surface and opening, around the cabinet's outer shape
  -> listening position    directly and, from Level 3, by way of the room
  => graphs, score, cost, and sound
```

A **radiator** is anything that turns one amplifier channel into sound. The first kind is a cone or dome driver mounted in the cabinet, together with the cabinet air behind it: box, ports, lines, and chambers, all taken from the 3D cabinet geometry. Section 5.9 adds DML panels, and section 3.3 keeps the door open for more.

"Digital isolated crossover" means every radiator has its own DSP channel and its own amplifier channel. Radiators are electrically isolated from each other. That is the default path and the one listen mode and export target. A design may instead use a **passive crossover** (section 7): one amplifier feeds an R/L/C network that splits the signal to the drivers, for kit copies and for users who want to simulate or later optimize a passive board.

### 2.2 What the user does

1. Picks drivers from a catalog of real parts with prices.
2. Shapes the cabinet: an outer shape that can be a plain box or a novel form, walls, chambers, ports, internal ducts and folds, bracing. Sees it in 3D. Gets print files for the printed parts and a cut list for the stock parts.
3. Places radiators on any surface, facing any way, including unconventional ones such as rear-facing DML panels.
4. Sets the crossover, or has the lab propose one.
5. Runs correction and auto EQ.
6. Reads the results: frequency response, impedance, cone travel, port air speed, and maximum clean level, before and after correction, per radiator and summed.
7. Listens to the simulated speaker and switches between designs.
8. Sees what the design costs and how it scores, and asks the optimizer for the cheapest design that meets a target.
9. Asks the lab to generate cabinets: whole designs it proposes, ranked, each one a design the user can open, change, print, and hear.

### 2.3 Why all parts are built together

The stages depend on each other in a line. Cabinet shape sets the air load on the driver. The load sets how the driver behaves. That sets what the crossover and correction must do. All of it sets the sound and the cost. A part built on its own gets built against guesses about its neighbors and has to be rebuilt. So every part exists in thin form from the first level, connected end to end, and all parts deepen together. Section 12 turns this into a rule.

### 2.4 Where the savings come from

- Digital processing replaces passive crossover parts and corrects response errors that would otherwise need better drivers.
- Printed geometry, where it helps, replaces woodworking and makes internal shapes possible that improve a cheap driver's result. Printing is an option, not a requirement: a part is printed only when that makes the sound better or the build cheaper.
- Drivers are chosen by simulated result per dollar, not by price or reputation.
- Cheap off-the-shelf materials such as PVC pipe, cardboard tubes, and foam board replace cabinet-grade wood where the physics allows, and printing is kept to the parts that need it.
- Every part is new, off the shelf, and has public data. Used, salvaged, or untested parts are out of scope.

The lab must also show the limits of these trades: what correction cannot fix, and where money has to go into hardware.

### 2.5 A playground stays open

New ideas arrive after the code exists: a new outer shape, a new internal element, a new kind of radiator. Each must be addable as a module without rewriting what is already there. Section 3.3 makes that a rule.

### 2.6 The lab generates

The owner's statement of intent (2026-11-12): "the program should be able to generate novel and complex 3D models of the inside and outside of the speaker cabinet with different components and component orientation and model the effects on the sound quality."

So generating is a goal of the product, beside checking what the user designs:

- **Outside.** The lab proposes outer forms, including curved ones, not only boxes.
- **Inside.** It proposes the inside: sealed, ported, folded and tapered lines, chambers, and plastic placed by the 3D model (section 5.5, "Shaping the inside").
- **Components.** It chooses drivers and other sound sources from the catalog, where each sits, and which way it faces.
- **Sound quality.** Every proposal is judged by the same simulation as a hand-made design (section 2.1), and the best are checked with the 3D models. A generator is only as good as the score it chases, so the score has to describe what is heard: off-axis and in-room sound as well as the level straight ahead, with the air's losses in the 3D model (sections 5.5 and 11).
- Every generated cabinet is an ordinary design file. Nothing about it is special once it is open.

Sections 11.4 and 12 say how this is built.

**How the industry builds this** (researched 2026-11-12, at the owner's request; sources in `STATUS.md`). Professional loudspeaker simulators share four habits, and the lab is to follow them:

1. **Any configuration.** A speaker is a list of sound sources, each with its own position, facing, and filter. Nothing in the simulator or the generator may assume a count or a layout.
2. **A network joined to a boundary-element model.** Drivers and filters are a fast network of lumped parts; the air inside and around the cabinet is solved once per frequency on a mesh of the cabinet's surfaces (the boundary element method), which gives the sound in every direction from one solve. The lab's 3D model works in time on a block of air and needs one run per direction, so a boundary-element model is the planned replacement for the outside (section 12, Level 3 track D). First slices in `seatlock-bem`: exterior CBIE on constant triangles, pulsating sphere (CHK-BEM-1) and rigid-sphere plane-wave surface pressures at low `ka` (CHK-BEM-2, toward CHK-RAD-3); not yet a radiation member or joined to the network. Higher-`ka` rigid-sphere scattering and Burton–Miller remain.
3. **A score from the whole sound field.** The standard measurement (CTA-2034, the "spinorama") takes 70 directions and forms the level straight ahead, the listening window, the early reflections, the sound power, and a predicted in-room response (12 % listening window, 44 % early reflections, 44 % sound power). A published rating predicts listeners' preference from the smoothness and slope of those curves and the bass limit. The lab's score is to be built on these curves (section 11.1).
4. **A missing detail never stops a design.** Where a number is unknown the tools carry on with what is known and say what is left out. The lab does the same: it warns, and refuses only what truly cannot be made.

## 3. Architecture

These decisions are fixed. Each has a reason. If a reason stops holding, raise it in `STATUS.md` before changing course.

| Decision | Reason |
| --- | --- |
| A local web app in a desktop browser. Reference machine: the owner's laptop, Linux Mint, 32 GB RAM, no discrete GPU assumed. Current Chrome and Firefox. | Heavy simulation needs a desktop processor. The browser provides 3D, graphs, and audio with nothing to install. Phone screens must not break, but heavy solves may be unavailable there. |
| The engine is Rust compiled to WebAssembly. All physics, DSP, geometry measurement, scoring, and optimization live in the engine. The one exception is the print solid (section 6.3). | Fast enough for the 3D solver. Strict types catch unit and shape mistakes. `cargo test` runs every physics check natively, without a browser. The same code can later run natively or on a playback device. |
| The interface is TypeScript and React, built with Vite. The 3D view uses three.js. | Widely used, well documented, well supported by coding agents. |
| The interface never computes physics. It edits the design and displays results. | One implementation of each formula. |
| The engine runs in Web Workers, never on the interface thread. Each request carries the design's revision number. Results for an old revision are discarded. Long jobs report progress and can be canceled. | The interface stays responsive during solves. |
| Real-time audio runs in an AudioWorklet that executes the engine's own DSP code. | What is simulated and what is heard come from the same code. |
| No server, no accounts, no network at run time. Designs are local files plus browser autosave. The catalog ships inside the app. | Works offline, nothing to host. The production build is a folder of static files. |
| Parallel work uses several workers, each with its own engine instance. Do not require SharedArrayBuffer or WebAssembly threads until a measured need exists. | Those need cross-origin isolation headers, which complicate hosting and development. |

Build and tooling rules:

- Build both WebAssembly modules with `cargo build --target wasm32-unknown-unknown --release` and no other build tool. `engine/.cargo/config.toml` turns on WebAssembly SIMD for that target, which every current Chrome and Firefox runs; it halves the 3D solver's run time. Both expose a plain C ABI (`extern "C"`): allocate, free, and one call that takes a method name and a JSON request and returns a JSON reply, with raw buffers for large arrays. Do not use `wasm-bindgen` or `wasm-pack`. Reason: the owner's laptop then needs only Rust and Node, with no extra build tool to install and keep at a matching version, and the workers and the worklet load their modules the same way.
- Engine dependencies must be pure Rust. The `wasm32-unknown-unknown` target cannot link C or C++ libraries without extra toolchains.
- The page fetches the real-time module's `.wasm` bytes and posts them to the worklet, which instantiates them there, because the worklet scope has no `fetch`. Only numbers and sample buffers cross that boundary, never text, because the worklet scope may lack the text-decoding APIs.
- Use the current stable release of each tool when you scaffold. Pin versions with `rust-toolchain.toml`, `Cargo.lock`, and `package-lock.json`, and record them in `STATUS.md`. List the `wasm32-unknown-unknown` target in `rust-toolchain.toml` so that rustup installs it by itself.
- Define the design types once, in Rust. Generate the TypeScript types from them. Do not maintain two hand-written copies.

### 3.1 Repository layout (target)

```
README.md          this specification
STATUS.md          current state, decisions, next steps, questions for the owner
COMS.md            handoffs between sessions and the board for helpers (section 1.4)
docs/new-chat.md   paste for opening a fresh lead chat (section 1.4)
AGENTS.md          pointer that sends coding agents to this file (CLAUDE.md imports it)
start.sh           the one command the owner runs
check.sh           the one command that runs every check
engine/            Rust workspace
  crates/
    seatlock-core        constants, units, complex helpers, frequency grids, design types, errors
    seatlock-geom        cabinet parameters -> air network, solid primitives, voxel grid, volumes
    seatlock-acoustics   radiators (cone driver, later DML panel), enclosure loads (lumped, waveguide), radiation, room, limits
    seatlock-fdtd        3D solver
    seatlock-dsp         filters, delay, FIR, limiter; frequency response and sample processing of each block
    seatlock-correct     targets, auto EQ, physics limits
    seatlock-sim         design -> results; score; cost; optimizer
    seatlock-wasm        C-ABI wrapper used by the workers
    seatlock-rt          C-ABI real-time kernel used by the AudioWorklet
web/               TypeScript and React app
catalog/           drivers/, exciters/, panels/, damping/, fills/, electronics/, printers/, stock/, materials.json
targets/           quality targets
designs/examples/  example designs used by checks and demos
docs/              archives: older COMS.md threads and STATUS.md history
tools/             cloud-check.sh: ./check.sh for Claude's cloud workspace (not for the owner)
```

Dependencies point one way, from `seatlock-core` up to `seatlock-sim`. `seatlock-dsp` and `seatlock-rt` never depend on physics crates. Nothing under `engine/` depends on the browser except the two wrapper crates. You may merge or split crates if you keep that direction and record why.

- `./start.sh` checks that the tools are installed and, if one is missing, prints the exact command to install it on Linux Mint. It builds the engine, installs web dependencies when needed, starts the app, and opens the browser. It must work from a fresh clone.
- `./check.sh` runs formatting, lints, `cargo test --workspace`, the WebAssembly build, the TypeScript type check, unit tests, and one headless-browser test that opens the app and loads an example design.

### 3.2 The engine's front door

```
simulate(design, request) -> results
  request:  { enclosure: "lumped" | "waveguide" | "fdtd",
              radiation: "point" | "baffle" | "exterior", space: "full" | "half" | "step",
              frequency grid, which outputs (sound_power), cell sizes (fdtd_dx_m, exterior_dx_m),
              fdtd_full_decay (run the 3D cabinet model to the end instead of stopping early),
              listening_window (section 11.1), losses (the air's wall losses, sections 5.4 and 5.5),
              walls (the cabinet's walls vibrate, section 5.11) }
  results:  per radiator and summed: complex pressure response, impedance, cone travel,
            opening air speed, limits, maximum level; score; cost; warnings;
            and for every curve the fidelity label of section 5.8
```

One function turns a design into results. The interface, the optimizer, the checks, and listen mode all use it. The other entry points (correct, optimize, export, impulse response) take a design and return a design, files, or samples.

### 3.3 Extension points

The lab is a playground, so new ideas will arrive after the code exists. Each family below has a contract and a registry. **Adding a member means adding its own files and one registration. It must not require editing the members that already exist.** CHK-EXT-1 enforces this.

| Family | A member must provide | First members (level) |
| --- | --- | --- |
| Outer shape | The print solid or stock parts; inside tests for its air and for the whole cabinet, or a mesh, for the voxel grid; named mounting surfaces that return a position and an outward direction for any surface coordinate; outline data for diffraction. | `box` (1), `prism`, `tube`, and `sheet_box` (2), `revolved` and `loft` (3), imported `mesh` (3, owner request 2026-11-12) |
| Internal element | Its air or solid region; its role in the air network (node, edge, or shunt); its primitives for the print solid and the voxel grid. | chamber, duct, solid (1 to 2), placed plastic `voxels` (3), fill, passive radiator (3) |
| Radiator | Its electrical impedance; its surface sources per volt (section 5.6): one net volume velocity, or a list of surface elements, each a place on the mounting surface measured from the mount point, an area, and a volume velocity; its limits; whether it pushes on the cabinet's air; what it needs from its mount; its lines for the fidelity label; the catalog parts it uses. | `cone` (1), `dml_panel` (3) |
| Enclosure load model | `Z_ab` and `H_k` (section 5.2). | `lumped` (1), `fdtd` (1), `waveguide` (2) |
| Radiation model | Pressure at any point per unit volume velocity of each surface source, and radiated sound power, from the sources' positions and the outer shape (section 5.6). | `point` (1), `baffle` (2), `exterior` (3) |
| Room model | The path from the speaker's radiation to the listening position (section 5.10): the mirror images of the listening position it needs the speaker's sound at, whether it needs the sound power, and what it adds as pressure and as energy. | `free` (1), `simple` (3) |
| DSP block | Its frequency response, its sample process function, its parameters. | biquad, crossover, delay, gain, limiter (1 to 2), FIR, dynamic bass (3) |
| Correction feature | Blocks with reasons, from responses, limits, and target, under the keep-only-if-it-helps rule. | C1 to C10 (section 8) |
| Catalog kind | Record shape, sourcing rule, tier, price. | driver, amplifier, DSP device, print material, stock material (1 to 2), exciter, panel material (3) |
| Exporter and importer | Files out and files in. | STL, filter text (1), 3MF (2), OpenSCAD program, FIR WAV (3), measurements and hardware formats (4) |
| Metric and target | A number computed from results, and its threshold. | section 11 |
| Optimizer strategy | A search over a declared set of variables. | section 11.3 |

How to build them:

- Every member has a `kind` string. In a design file, a part that comes in kinds is `{ "kind": "...", ...its own fields }`.
- Do not model a family as one closed list in code, such as a single Rust enum that every addition has to edit. Keep each part as its `kind` plus raw fields until the registered member for that kind parses it.
- A design that uses a kind this build does not know still opens and still saves unchanged. The unknown part is shown as unsupported, the rest is simulated, and a warning names what was left out. CHK-EXT-2 enforces this.
- A new member arrives with its checks, its fidelity label text, its glossary entry, and its place in the interface. If it adds fields to the design file, it also raises `schema_version` and adds a migration.
- When an idea does not fit a contract, extend the contract in a way that keeps existing members working, and record the change in `STATUS.md`.

## 4. Conventions

- **SI units everywhere in the engine and in design files**: m, m^2, m^3, kg, s, Hz, Pa, N, V, A, ohm, H. Field names end with the unit: `vas_m3`, `xmax_m`, `fs_hz`, `re_ohm`. Convert only at the screen (liters, mm, cm^2, grams) and at file export.
- **Constants**, defined once in `seatlock-core`, for air at 20 °C and 101.325 kPa: `RHO0 = 1.204 kg/m^3`, `C0 = 343.2 m/s`, `P_REF = 20e-6 Pa`.
- **Amplitudes are RMS** unless the name ends in `_peak`. Time dependence is `e^(+j*omega*t)`. `omega = 2*pi*f`, `k = omega / C0`.
- **Frequency grid**: logarithmic, 10 Hz to 24 kHz, 48 points per octave, unless a request says otherwise.
- **Sample rate**: 48 kHz by default. Ask the audio context for the design's rate, read back the rate it actually gives, and compute coefficients for that rate.
- **Number types**: `f64` for analysis and for filter coefficients and filter state. `f32` for audio samples and for the 3D solver's fields.
- **Real-time code allocates nothing, takes no locks, and makes no system calls** inside the processing function.
- **Coordinates**: right-handed, in meters. `x` runs left to right when facing the front baffle, `y` front to back, `z` bottom to top. The origin is the outer front-bottom-left corner. The listener is on the `-y` side. A position on a mounting surface is given in that surface's own coordinates, which the outer shape defines. On a flat face they are `[u, v]` from the face's lower-left corner as seen from outside.
- **Determinism**: the same design and request give the same results. Anything random, such as optimizer starting points, takes a seed stored in the request.
- **Errors and warnings are data**: a code, a plain-language sentence, and the design field they refer to. The interface shows them beside that field.

### 4.1 The design file

A design is one JSON file, `name.seatlock.json`. This is the Level 1 shape. Later levels add fields. When the shape changes, raise `schema_version` and add a migration so older files still open.

```jsonc
{
  "schema_version": 1,
  "name": "sealed-test",
  "speaker": {
    "enclosure": {
      "outer": { "kind": "box", "width_m": 0.160, "height_m": 0.250, "depth_m": 0.180, "wall_m": 0.008, "edge_radius_m": 0.0 },
      "chambers": [],        // empty: the whole interior is one chamber with id "main"
      "ducts": [],
      "mounts": [ { "id": "m1", "surface": "front", "at_m": [0.080, 0.150], "rear_into": "main", "recess_m": 0.0 } ],
      "solids": []
    },
    "radiators": [ { "kind": "cone", "driver_id": "test-woofer-a", "mount": "m1" } ],   // ordered from lowest range to highest
    "listening": { "distance_m": 1.0 }
  },
  "dsp": {
    "sample_rate_hz": 48000,
    "whole_speaker": [],     // blocks applied before the split into radiators
    "per_radiator": [
      { "blocks": [ { "kind": "biquad", "type": "highpass", "f_hz": 35.0, "q": 0.707 } ],
        "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null }
    ],
    "correction": { "enabled": true, "protected_band_hz": [3000, 5000], "max_filters": 10 }
  },
  "electronics": { "amplifiers": ["test-amp-a"], "dsp_device": null },   // one amplifier channel per radiator
  "build": { "material": "pla", "infill_fraction": 1.0, "stereo_pair": true, "extras_cost": 0.0 },
  "target": "hifi-default"
}
```

## 5. Physics

### 5.1 Cone driver, radiator kind `cone`

The catalog stores what the manufacturer publishes: `fs_hz, qes, qms, vas_m3, re_ohm, le_h, sd_m2, xmax_m, pe_w`. Derive the mechanical parameters from those:

```
cms = vas / (RHO0 * C0^2 * sd^2)        suspension compliance, m/N
mms = 1 / ((2*pi*fs)^2 * cms)           moving mass including air load, kg
rms = 2*pi*fs*mms / qms                 mechanical resistance, N*s/m
bl  = sqrt(2*pi*fs*mms*re / qes)        force factor, T*m
qts = qes*qms / (qes + qms)
```

If the source also prints `bl`, `mms`, or `cms` and one differs from the derived value by more than 10 %, keep computing from the list above and attach a warning to the driver.

Some data sheets, mostly for tweeters, do not print `vas_m3`. Then derive `mms` from the first one printed, in this order: `mms_kg` as printed, or `bl_tm` through `mms = bl^2 * qes / (2*pi*fs*re)`. Then `cms = 1 / ((2*pi*fs)^2 * mms)`, and the rest as above. If neither is printed but the sheet prints a sensitivity, derive `vas_m3` from it. Store the sensitivity as printed, in `sens_db_1w` (SPL at 1 m for 1 W) or `sens_db_2v83` (SPL at 1 m for 2.83 V), then:

```
sens_db_1w = sens_db_2v83 - 10*log10(2.83^2 / re)
eta0       = 10^((sens_db_1w - K_half) / 10)       K_half = 10*log10(RHO0*C0 / (2*pi*P_REF^2)) = 112.16 dB
vas        = eta0 * C0^3 * qes / (4*pi^2 * fs^3)
```

This inverts the CHK-DRV-2 formula and treats the printed figure as half-space, which is close to the large baffle makers usually measure on. It is the least reliable path, because makers often average sensitivity over a band, so the driver is labeled "derived from sensitivity". For a sealed-back driver it still fixes everything the model uses: the level, the shape from `fs` and the Q values, and cone travel through `sd`. When a sheet prints both `vas_m3` and a sensitivity, and the level computed from `vas_m3` differs from the printed sensitivity by more than 1.5 dB, keep `vas_m3` for compliance and the box shape, attach a warning, and keep the Vas-derived level. When an **independent** measurement prints a sensitivity that disagrees the same way (owner 2026-11-15), still keep `vas_m3` for compliance, but scale the radiated volume velocity so free-air half-space level matches that independent figure; the fidelity label names both. Cone travel, current, and impedance stay on the Vas path. Record which path was used. If `xmax_m` is not printed, `max_spl` leaves cone travel out for that driver, and a warning says so beside the maximum clean level.

With voltage `V` at the terminals, and the enclosure presenting acoustic impedance `Z_ab` to the back of the cone (section 5.2):

```
Ze  = re + j*omega*le
Zm  = rms + j*omega*mms + 1/(j*omega*cms) + sd^2 * Z_ab
u   = bl*V / (Ze*Zm + bl^2)             cone velocity, m/s
Zin = Ze + bl^2 / Zm                    electrical impedance, ohm
I   = V / Zin
U_d = sd * u                            volume velocity leaving the front of the cone, m^3/s
x_peak = sqrt(2) * |u| / omega          cone travel, m
```

The air load on the front of the cone is left out of `Zm` because `mms` already includes it. A driver with a sealed back, such as most tweeters, has `Z_ab = 0` and no openings.

### 5.2 The enclosure load interface

Every enclosure model, whatever its fidelity, answers the same two questions. `U_in` is the volume velocity that the back of the cone pushes into the enclosure.

```
Z_ab(f) = p_back / U_in       acoustic impedance seen by the back of the cone, Pa*s/m^3
H_k(f)  = U_out_k / U_in      volume velocity leaving opening k, per unit pushed in
```

When the cone moves outward, `U_in = -U_d`. The total volume velocity this driver sends into the room is therefore

```
U_total = U_d * (1 - sum over k of H_k)
```

A sealed box has no openings, so `U_total = U_d`. In a vented box `H -> 1` as frequency goes to zero, so `U_total -> 0`: the port cancels the cone.

Each driver sees the load of the chamber it pushes into (as built, Level 3): the network is solved once from each chamber a driver pushes into. Chambers not joined by a duct do not affect each other, so a midrange in its own sealed chamber is exact; chambers joined by a duct are each solved as if the other chamber's driver were still, and the label says so. Drivers that share a chamber share its air: each sees the chamber's `Z_ab` times their number, which is exact when they are the same model on the same processing (two woofers on one channel's settings) and an approximation otherwise, with a label line either way. The full 3D model still drives only the first driver's chamber.

Three models implement this interface: `lumped`, `waveguide`, and `fdtd`. The driver model, radiation, limits, correction, and listen mode do not care which one is in use. That is what lets a fast model drive the interactive view while a slow one refines the same curves in the background.

### 5.3 Model `lumped`

```
C_ab = V_box / (RHO0 * C0^2)                       box air compliance

sealed:  Z_ab = 1 / (j*omega*C_ab)

vented:  M_ap   = RHO0 * L_eff / S_port            port air mass
         Z_port = j*omega*M_ap + R_ap
         Z_ab   = 1 / (j*omega*C_ab + 1/Z_port + 1/R_leak)
         H_port = Z_ab / Z_port
         f_b    = (C0 / (2*pi)) * sqrt(S_port / (V_box * L_eff))
         L_eff  = L + one end correction per end
                  end correction = 0.849*a for an end flush with a wall, 0.613*a for a free end
                  a = sqrt(S_port / pi)
         R_leak = q_leak / (2*pi*f_b*C_ab),  q_leak = 7 by default
```

`V_box` is the net air volume: the gross internal volume minus what the driver, port, and bracing displace. Several ports from one chamber are parallel branches. Checks run with no leak and `R_ap = 0`.

This model knows volumes and ports. It knows nothing about shape: no standing waves, no folds, no pipe resonances in the port.

### 5.4 Model `waveguide`

The cabinet's air is a network of ducts and chambers (section 6.2). A duct segment of length `L` and area `S` relates pressure and volume velocity at its two ends:

```
[p1]   [ cos(k*L)          j*Z0*sin(k*L) ] [p2]
[U1] = [ j*sin(k*L)/Z0     cos(k*L)      ] [U2]          Z0 = RHO0*C0 / S
```

- A tapered duct is sliced into uniform segments no longer than 10 mm.
- A chamber whose largest dimension is under one eighth of a wavelength is a shunt compliance with admittance `j*omega*V / (RHO0*C0^2)`. A longer chamber is a duct along its longest axis.
- Where a duct opens into a chamber through a wall, add `0.849*a` to its length.
- An opening to outside air ends in its radiation impedance: `Z0 * ((k*a)^2/4 + j*0.613*k*a)` for a free end, `Z0 * ((k*a)^2/2 + j*0.849*k*a)` for an end flush with a wall. Both hold for `k*a < 1`.
- A bend is treated as a straight duct of the center-line length. Report that as an approximation (section 5.8).
- Losses arrive at Level 3: replace `k` by `k - j*alpha` with `alpha = 2.99e-5 * sqrt(f) / a` per meter for a smooth round duct, and add a flow-resistance term for damping fill.

Solve the network for `Z_ab` at the driver and `H_k` at each opening. Tree-shaped networks are enough through Level 2. Loops come at Level 3.

Losses as built (Level 3): a request with `losses` replaces `k` by `k - j*alpha` in every duct slice, with `alpha = 2.99e-5 * sqrt(f) / a` and `a` the radius of the round duct of that slice's area. A slot, a rough printed wall, and a bend lose more than that, and the fidelity label says so. Chambers stay lossless compliances, and damping fill is not built. The page, scores, the optimizer, and the generator ask for losses; the engine's default, which the numbered checks use, does not. CHK-WG-4.

As built (Level 3): the network is a tree grown from the driver's chamber. Each duct is a line along its center line, whatever its bends, with its taper sliced; it ends in its mouth's free-end radiation load or in the chamber at its far end, which carries its own ducts in turn. Every end that opens into a chamber adds `0.849*a`. Chambers are compliances. A duct may be written in either direction; the line is the same. A network with a loop is refused by this model, and the quick model's curves are used with the reason in the fidelity label. The label also counts the bends treated as straight. Checked against the full 3D model on the `folded-line` example (a 1.02 m slot, folded three times, tapering from 80 to 60 mm): the bass roll-off within 4 %, the line's first deep dip within 2 %, and the level within 1 dB from 50 to 150 Hz.

This model knows lengths, tapers, quarter-wave lines, and pipe resonances. It knows nothing across a duct: bends, sudden steps, and chamber standing waves in the other two directions are invisible to it.

### 5.5 Model `fdtd`, the 3D solver

A finite-difference time-domain solution of the linear acoustic equations on a grid of cubic cells that fills the cabinet's air.

```
cell size dx; pressure at cell centers; velocity on cell faces
dt = 0.95 * dx / (C0 * sqrt(3))
velocity:  v += -(dt / (RHO0*dx)) * (p_ahead - p_behind)        only on faces between two air cells
pressure:  p += -kp * (net outflow through the cell's faces),   kp = RHO0*C0^2*dt/dx
rigid wall: the face velocity stays 0
```

The rules below were found while validating this specification with a reference solver. Follow them unless you can show something better.

- **Fold boundary admittance into the pressure update.** For a cell whose boundary faces have total specific admittance `Y_sum` (each absorbing face contributes `1 / (RHO0*C0)`):
  ```
  g     = 0.5 * kp * Y_sum
  p_new = (p*(1 - g) - kp*net_interior_outflow + kp*v_source) / (1 + g)
  ```
  Setting boundary face velocities directly from the old pressure goes unstable in corner cells near the stability limit.
- **Drive through a resistive source.** The driver aperture's faces are boundary faces with admittance `1 / (RHO0*C0)` and a source velocity `v_source(t)`, a short Gaussian pulse. The net inflow velocity on each face is `v_net = v_source - (p_new + p) / (2*RHO0*C0)`. Record the aperture's mean of `(p_new + p)/2` and `U_in = sum(v_net) * dx^2` at every step. Then `Z_ab = FFT(p) / FFT(U_in)` and `H_k = FFT(U_out_k) / FFT(U_in)`, where `U_out_k` is the flow through a plane across opening k. The absorbing source drains the cavity quickly, so runs are short, and the ratios still give the true impedance. A rigid, lossless cavity driven by a hard source rings almost forever.
- **Openings radiate into an exterior air block** whose outer faces absorb. The block extends five opening widths past the mouth in every direction, and doubling it changes the resonance frequencies, and `Z_ab` and `H_k` more than 5 % away from any resonance (a maximum or a minimum of `|Z_ab|`), by under 1 %. Three widths, the first choice, moved them by up to 1.4 %. An absorbing face takes the admittance of an outgoing spherical wave centered on the mouth, `Y = (1 + 1/(j*k*r)) / (RHO0*C0)`, so its velocity is `p/(RHO0*C0) + (1/(RHO0*r)) * integral of p dt`, with `r` at least two cells. A plane-wave face (`1/(RHO0*C0)` alone) is resistive at every frequency, while the near field of a small opening is mostly reactive at low frequency; with plane-wave faces the low-frequency `Z_ab` changed by up to 23 % when the block was doubled. Do not judge this by peak heights. Through Level 2 the air is lossless, so a port resonance is damped only by radiation: `Q = 2*pi*L_eff / (k*S)`, about 450 for the CHK-FDTD-3 neck, a bandwidth of 0.15 Hz. Its height then depends on the absorbing faces and on the run length, not on the cabinet. Label resonance peak heights as not modeled until losses arrive at Level 3.
- **Stop** when the aperture pressure has fallen below 1e-4 of its peak, or at a time cap.
- **Stop early by harmonic inversion** (as built, Level 3). After the source pulse every recorded signal is a free decay: a sum of damped sinusoids whose poles are shared. From 30 ms after the pulse, every 10 ms of simulated time, the poles are found by the matrix pencil method on the last 120 ms of the signals (low-passed to 1.35 times the highest frequency asked for and decimated), each signal's amplitudes are fitted, and the rest of its Fourier sum is added in closed form: `sum over n >= N of c*z^(n-n0)*e^(-j*w*n*dt) = c*z^(N-n0)*e^(-j*w*N*dt) / (1 - z*e^(-j*w*dt))`. The run stops when three fits in a row agree: `Z_ab` and each `H_k`, power-averaged over a sixth of an octave, within 0.05 dB, each measured against what it adds to (`Z_ab` against `|Z_ab|` plus the driver's own `Z_m / S_d^2`, found from two evaluations of the radiator, and `H_k` against `|H_k|` plus 1, the cone's own output). The heights of single sharp resonances are not part of the test, because without losses they are set by the run's length (this section, above). Against running until the echoes die: `two-way-vented` within 0.020 dB in 12,500 steps instead of 41,320, `folded-line` within 0.023 dB in 14,375 instead of 47,438, `sealed-test` within 0.002 dB. A request with `fdtd_full_decay` runs to the end instead, for checks.
- **Valid range**: up to `C0 / (10*dx)`, which is 3.4 kHz at 10 mm and 6.9 kHz at 5 mm. A feature thinner than two cells is unresolved, so warn about it.
- **Preserve cross-section areas** when turning round or slanted shapes into cells. A duct along a grid axis gets, slice by slice, as many cells as its true area holds, nearest its axis first; its printed wall, usually thinner than a cell, is closed by making every cell beside the bore solid along the duct's length. The grid is shifted along each axis so the cells across the cabinet's air, on a line through its middle, add up to the air's true length. The driver's body is a solid cylinder of its cut-out's radius and its published displacement behind the wall, and the source is the air against the wall or that body within the cone's radius. Slanted and curved walls become stair-steps, so run at two cell sizes and report the difference as the uncertainty.
- **Where the model hands over.** The 3D result replaces `Z_ab` and `H_k` up to its valid range; above it, the `waveguide` result is used and the fidelity label says so.
- **Self-check every run**: below `C0 / (50 * longest internal dimension)`, `Z_ab` must match the detailed (or quick) model's `Z_ab` within 3 %, or the result carries a warning. In a cabinet with ports `Z_ab` there is the ports' air mass, so the difference also holds the end corrections and flares that the detailed model only estimates (5.0 % for the CHK-FDTD-4 cabinet at 5 mm; about 25 % for the LCM's flared moulded port at 5 mm).
- **Time budgets** on the reference machine with one worker: a 20-liter cabinet in under 20 s at 10 mm for a preview and under 5 min at 5 mm for a final result. These are targets. Record measured times in `STATUS.md`.

- **Wall losses** (as built, Level 3). At a wall, sound loses a little to the air's friction and heat flow. For sound grazing the wall the loss is the duct's of section 5.4: a specific admittance whose real part is `B = 2.99e-5 * sqrt(f)`. Friction is `1/1.4737` of that and counts with `sin^2` of the angle from the wall's normal; heat flow is the rest and counts at every angle. Sound arriving from all directions alike loses on average `(2/3 + 0.4737)/1.4737 = 0.774` of the grazing value, which makes every resonance decay at `s(f) = 0.774 * C0 * B(f) * S/V` per second, with `S/V` the cabinet's wall area over its air volume, both counted on the grid. A request with `losses` applies it as a decay weight: each signal's Fourier sum at frequency `f` is weighted by `e^(-s(f)*t)`, which is exactly the response of the same grid with a uniform loss `s(f)` in its air, each resonance `s/pi` Hz wide. It costs nothing, works with the early stop (the fitted tail takes the same weight), and makes peak heights finite and independent of the run's length. It is one average: a single resonance's own loss differs from it by up to about half (0.65 of it for a wave grazing every wall, as in a duct), and stair-stepped walls count a little large. The label says so. CHK-FDTD-5.
- **Damping material** (as built, Level 3, for imported `damping` parts). A cell of damping material slows the air by a flow resistivity `sigma` (Pa*s/m^2), the Rayleigh model: the velocity update becomes `v_new = ((1 - b)*v_old - kv*dp) / (1 + b)` with `b = sigma*dt/(2*RHO0)`, stable for any `sigma`. A face takes the mean of its two cells' figures. Checked against a lossy duct's closed form (`k_c = k*sqrt(1 - j*sigma/(RHO0*omega))`, `Z_c = RHO0*C0*k_c/(k*S)`), at the half cell where the grid records pressure: within 0.14 %. One figure at every frequency (real fibre's resistance and its added air mass change with frequency), and only the full 3D model sees it: the quick and detailed models treat damping as air, and the label says so.

This model knows the full shape of the internal air, within its grid. The walls are rigid in the grid; their loss is the average above when asked for, and otherwise none. A request with `walls` adds the walls' vibration to its result as a shunt (section 5.11). It does not model damping fill except imported damping parts.

**Shaping the inside** (as built, Level 3). The 3D model can work out where plastic inside the cabinet evens out the sound.

- **Relaxed faces.** A face between two air cells takes an open fraction `a` from 0 (closed) to 1 (open) in the velocity update, `v -= kv * a * (p_ahead - p_behind)`. 0 and 1 reproduce the plain grid bit for bit.
- **Gradient.** The scheme is linear, and its system matrix is symmetric, so it is its own adjoint: `dZ_ab/da` for every face comes from the forward run alone, from the Fourier sum of the pressure difference across the face, and `dH_k/da` from one more run of the same scheme per opening, driven at the opening's plane. The formulas are in `seatlock_fdtd::adjoint`. Against central finite differences: within 0.12 % at open fractions 0, 0.5, and 1.
- **A uniform loss, not the early stop.** Air sealed behind a nearly closed face rings almost forever without losses, and the early stop's tail fit would be needed for every face's sum. Instead every signal is weighted by `e^(-s*t)`, which is exactly the same cabinet with a uniform loss in its air (every resonance widened by `s/pi` Hz; `s = 100 /s` by default, 32 Hz). A run of `ln(1/1e-5)/s` seconds then holds everything, whatever the shape. Real losses are not modeled yet, so the result says which loss it assumed.
- **Objective.** The unevenness of the level across a band (150 Hz to 1.5 kHz by default, or the grid's limit): the RMS difference, in dB, from the band's mean, at frequencies no farther apart than a resonance is wide. The level is that of the summed volume velocity `U_d * (1 - sum of H_k)` through the driver's own model; the baffle and the room, which the inside does not change, are left out. Its gradient through the driver matches finite differences within 0.001 % of the largest value.
- **Optimizer.** Each cell of chamber air (not within 30 mm of the cone, not at an opening) has a density from 0 (plastic) to 1 (air); a face is open by the product of the densities on its two sides. First a density stage: a smoothing filter of 1.5 cells, a projection that sharpens in steps, a cap on the plastic (15 % of the region), starting from an even haze of plastic, with backtracking steps. Then a finishing stage on the plastic-or-air shape itself: the gradient names the cells at the plastic's surface whose change should help most, a batch is changed, the shape is cleaned for printing (no plastic thinner than two cells, none that hangs in the air, no air sealed off from the driver, no blocks that touch along an edge or at a corner only), and the change is kept only if a fresh run is more even.
- **Result.** A `voxels` solid in the design (section 6.2), which every model, the print file, and the cost take up. Measured on `sealed-test` made 0.5 m tall: 0.472 to 0.420 dB at 20 mm cells and 0.478 to 0.378 dB at 10 mm with 1.14 L of plastic. The gain is modest: a plain sealed box with this loss is already within half a dB, and turning part-air cells into plastic gives back about half of what the density stage finds (0.307 dB at 10 mm).

### 5.6 Radiation and summation

Every radiator hands the radiation model a list of **surface sources**. A surface source is a patch of moving surface: a position, an outward unit direction, an area, and a complex normal velocity per volt. Its volume velocity is `U_s = velocity * area`.

- A cone is one source at its mount, facing along the mount's outward direction, with `U_s = U_d`.
- Opening k of that cone's enclosure is one source at the opening, with `U_s = H_k * U_in = -H_k * U_d`. Keep that sign: it is what makes a port cancel the cone at low frequency.
- A DML panel is a grid of sources (section 5.9).

A radiation model turns the sources and the cabinet's outer shape into pressure at any point and into radiated sound power. There are three. A radiator does not care which one is in use.

**`point`, Level 1.** Every source is a simple source:

```
p_s     = j*omega*RHO0 * U_s * B(f) * e^(-j*k*r_s) / (4*pi*r_s)
p_total = sum of p_s over all sources of all radiators, each multiplied by its DSP and amplifier transfer function
SPL     = 20*log10(|p_total| / P_REF)
```

`B(f)` is the baffle gain, from 1 when sound radiates all around to 2 when it radiates forward only. The request's `space` picks it. `full` is `B = 1`. `half` is `B = 2`, an infinite baffle, and is what the checks use. `step` is a shelf centered on `f3 = 115 / baffle_width_m`: `B = (1 + j*f/(f3/sqrt(2))) / (1 + j*f/(f3*sqrt(2)))`. All sources sit at one point, the middle of the cabinet's front, and the direction a source faces is ignored. A design with a source that does not face the listener gets a warning under this model. Behind that point (the side away from the listener) the gain is `2 - B`, so half space sends nothing backward and full space the same both ways; the sound power is then `RHO0*C0*k^2*|U|^2 * (|B|^2 + |2 - B|^2) / (8*pi)`, the simple-source value for `B = 1` and twice that for `B = 2`.

**`baffle`, Level 2.** Each source sits at its real position. A source on a flat surface gets, at any point in front of that surface, piston directivity, `D = 2*J1(k*a*sin(theta)) / (k*a*sin(theta))` with `theta` measured from its outward direction, and edge diffraction computed from that surface's outline. A source on a curved surface, or one the shape cannot place, falls back to `point`, and the fidelity label says so.

Edge diffraction, the simple secondary-source model: divide the outline of the surface the source sits on into `N` equal edge elements, at least 4 per wavelength at the highest frequency and never fewer than 64. Element `i` is at distance `d_i` from the source and `e_i` from the listening point; the source is at distance `r_0` from the listening point. Then

```
p = p_half_space * ( 1 - (1/(2N)) * sum over i of (r_0 / e_i) * e^(-j*k*(d_i + e_i - r_0)) )
```

where `p_half_space` is the source's pressure with `B = 2`. At low frequency the sum tends to `-1/2`, so the level falls 6.02 dB to that of full space. At high frequency the terms cancel and the level approaches half space. The model ignores the depth of the cabinet and the edge's shape, which the fidelity label lists.

Behind the source's own surface (a driver on the back heard from the front, or any source heard from the wall behind the cabinet) the direct sound does not arrive, and the same edge elements send what the front loses, with the opposite sign:

```
p = p_half_space * (1/(2N)) * sum over i of (r_0 / e_i) * e^(-j*k*(d_i + e_i - r_0))
```

At low frequency that is the full-space level, the same as in front; at high frequency the terms cancel and the surface shadows its back. This is the same simple model, so it still ignores the cabinet's depth, and the fidelity label says so.

More than 32 sources on one surface (a panel's elements) share one edge term: the sum above is computed once, from the middle of those sources, and each source keeps its own distance and piston directivity. The fidelity label says so. Without this each of a panel's hundreds of elements needs its own sum over the outline at every frequency, which is far too slow for the page.

Sound power under `baffle` is the far-field integral below, taken over 48 directions spread evenly over the sphere. With up to eight sources the geometry's part (how each pair of sources adds up over all directions) is computed on every sixth frequency and interpolated, then combined with the sources' volume velocities on the full grid.

**`exterior`, Level 3.** The 3D solver of section 5.5 run on the air outside the cabinet. The outer shape is a rigid body inside a block of air with absorbing faces, and the surface sources are velocity boundaries on its skin. This is the model for novel outer shapes and for radiators that face sideways, upward, or backward. One reciprocity run, with a simple source at the listening position and the pressure recorded on the cabinet's skin, gives the transfer from every surface source to that position at once: `p(listener) / U(skin point) = p(skin point) / U(listener)`.

As built:

- **Only the scattered field is on the grid.** The listening position is usually outside the block, and its source's own wave is known in closed form: `p_inc = RHO0 * Q'(t - r/c) / (4*pi*r)` and `v_inc = r_hat * (Q'(t - r/c) / (4*pi*r*c) + Q(t - r/c) / (4*pi*r^2))`, with `Q(t)` a Gaussian pulse. Every face between the body and the air is given the velocity `-v_inc . n`, so the total normal velocity on the rigid skin is zero, and the total pressure is the closed-form incident pressure plus the grid's scattered pressure. At low frequency the scattered part is small, so the result is exact where the grid's outer faces are weakest. A point farther than 20 m sends a plane wave (used for sound power).
- **The outermost layer of cells absorbs** an outgoing spherical wave centered on the body's middle: `dp/dt + (c / cos) * dp/dn + (c / r) * p = 0` along the inward axis, in Mur's box form between the cell and its inward neighbor, with `cos = r_hat . n` the angle at which that wave crosses the layer (kept at 0.58 or more). The folded admittance of section 5.5 takes the pressure half a cell from the face, which reflects about 14 % at the grid's limit; that is harmless inside a cabinet at low frequency and too much here. With this layer, a moving face on one solid cell matches a simple source in free space within 5 % up to 2 kHz at 10 mm, and moving the outer faces from 0.3 m to 0.5 m changes nothing by more than 2 %.
- **The block** reaches 0.2 m past the cabinet's bounding box on every side. A cell is solid when its center is inside the outer shape (`contains_cabinet`). Valid up to `C0 / (10*dx)`; above that the `baffle` model's value is used as it is, and the fidelity label says so. The request's `exterior_dx_m` sets the cell, 10 mm by default.
- **A source's skin cells** are the air cells that touch the body within the source's radius of its position, between 0.75 cells behind and 1.75 cells in front of its surface, with the body behind them as the source sees it; a source smaller than a cell takes the nearest one. The source's transfer is the mean over its cells. A source the shape cannot place keeps the `baffle` model's value.
- **Sound power** is 14 plane-wave runs, from the six axis directions (weight 1/15) and the eight diagonals (weight 3/40), then `|p|^2 / (RHO0*C0)` summed over the far sphere.
- **Cost**: about 0.4 s per run at 10 mm for a 7-liter box in a native test build. A design with a room needs 16 runs (the listening position, its mirror image, and the 14 directions).

What validated it (CHK-RAD-3, CHK-RAD-4, and three more tests beside them):

- A rigid sphere of radius 0.1 m against the closed-form series, from `k*a = 0.18` to 6: where the wave arrives and at the side, within 0.75 dB at both cell sizes. In the deep shadow behind the sphere the level reads high near the grid's limit, by up to 3.4 dB at 10 mm and 1.7 dB at 5 mm. The error halves with the cell. It was first put down to stair-stepped cells on a curved surface; the trial of a grid that follows the surface (below) removed only a quarter of it at 10 mm, so most of it has another cause.
- Reciprocity on a box: the reciprocity run against a plain radiation run (a skin face moving, pressure recorded at the far point), beside the box and behind it: within 7 % and 10 degrees from 50 Hz to 3 kHz. The plain run carries the grid's dispersion over the whole path, so it is the less exact of the two.
- The CHK-RAD-2 arrangement in 3D (a source at the middle of a disc of radius 0.1 m, 10 mm thick): -6.02 dB at 10 Hz, +3.88 dB at 1716 Hz, -6.54 dB at 3432 Hz against half space, within 0.75 dB of the simple edge model at five frequencies. Two unrelated models agree.
- A body far smaller than the wavelength changes nothing for a far listener (within 0.05 dB at 10 and 20 Hz). For a listener at 1 m the near face of a small box reads about 0.7 dB above a simple source, which is real: a source on a body acts from a point a little in front of it.

A grid that follows the surface (tried, not in use). `Domain::around_smooth` keeps the cells a curved surface cuts, with the air fraction of each cell's volume and of each face's area: a face's velocity update is scaled by its fraction, a cell's pressure update is divided by its fraction (never below a sixth of the sum of its faces' fractions, which keeps the plain time step stable), and the body's share of a face carries the prescribed velocity. On the CHK-RAD-3 sphere it is stable, and the worst error in the shadow falls from 3.42 to 2.58 dB at 10 mm and from 1.67 to 1.55 dB at 5 mm, while the lit point goes from 0.67 to 1.02 dB and the side from 0.24 to 0.56 dB at 10 mm. That is not a clear gain, so the simulator still uses stair-steps. It also shows that most of the shadow error is not the stair-steps themselves; the grid's dispersion along the path around the body is the likely rest, which is not yet tested.

Not modeled: openings as openings (a port's mouth is a patch of rigid skin that moves), and anything that needs smooth curved surfaces in deep shadow.

Sound power is the integral of `|p|^2 / (RHO0*C0)` over a sphere in the far field. The results give it as a level in dB re 1 pW, with the directivity index: the on-axis intensity at the listening distance against the mean over all directions, `10*log10(|p_axis|^2 * 4*pi*r^2 / (RHO0*C0*W))`. Both are computed when the design has a room, or when the request sets `sound_power`.

The piston model has no cone break-up. Above roughly `k*a = 1`, a driver's real response comes from its published or measured curve. Level 2 adds a response file per driver, blended in above a chosen frequency. Until a curve is present, label that region accordingly (section 5.8).

As built: a driver record's `response_file` names a curve in `catalog/drivers/responses/`, the maker's own on-axis measurement copied from its published data file (`points` of `hz` and `db`, with the file's address, revision date, `read_on`, and measuring conditions). Above `f = C0 / (2*pi*a)`, with `a` the cone's radius, the level straight ahead keeps the model's value at that frequency and then follows the curve's shape, with the model's phase; above the curve's last point it holds the last value. Two limits, both in the fidelity label: the curve includes the maker's own test baffle, which takes the place of this cabinet's edges above the hand-over, and the listening window's side directions, the room, and the sound power still use the piston model. Twelve Dayton Audio drivers have curves (from the maker's Graphs/FRD/ZMA packages on Parts Express; eleven read 2026-11-12, ND25FW-4 on 2026-11-15). The RS225-4's package holds another driver's data and is not used; no package was found for the DC28F-8.

### 5.7 Limits and maximum level

Everything scales linearly with drive voltage, so compute at 1 V and scale.

```
cone travel         x_peak(f)                      <= xmax_m
opening air speed   sqrt(2) * |U_out_k(f)| / S_k   <= 0.05 * C0   (17 m/s, default)
voice-coil heating  |I(f)|^2 * re_ohm              <= pe_w
amplifier           V(f)                           <= sqrt(amp_power_w * amp_rated_load_ohm)
```

`max_spl(f)` is the SPL at 1 m when the first limit is reached with a sine at frequency `f`. For the whole speaker, the input rises until the first radiator reaches one of its limits, each radiator receiving the input through its own DSP chain; `max_spl(f)` is the summed level at that input. A woofer far above its crossover therefore never sets the speaker's limit. `headroom(f) = max_spl(f) - design_level`, where the design level is the target's `max_spl_db_min` (section 11). The limits above belong to the `cone` radiator and its enclosure. Every radiator kind states its own (section 3.3).

### 5.8 Fidelity labels

Every curve and number carries the models that produced it: the enclosure model (`lumped`, `waveguide`, `fdtd`, or `measured`), the radiation model (`point`, `baffle`, or `exterior`), and the room model. It also carries a list of design features those models ignored, for example "two bends treated as straight", "no cone break-up above 2.1 kHz", "chamber standing waves not modeled", or "rear-facing source treated as a point". The interface shows the label on every graph. When two models are available, draw both. Where they disagree is something the user needs to see.

### 5.9 DML panels, radiator kind `dml_panel`, Level 3

A distributed-mode loudspeaker is a stiff, light panel driven by an exciter. It radiates through many bending resonances at once, which spreads sound widely. A rear-facing panel reaches the listener mostly by way of the wall behind the speaker, so it can only be judged with the `exterior` radiation model and the room estimate of section 5.10.

The sealed-back panel is built and validated ("As built" below). An open-backed panel is not built yet, and a design that asks for one is refused with that reason.

The panel is a thin plate in bending, with deflection `w`:

```
D = E*h^3 / (12*(1 - nu^2))            bending stiffness, N*m
m = rho*h                              mass per area, kg/m^2
D * del^4(w) + m * d2w/dt2 = pressure difference across the panel + exciter force per area
simply supported panel a by b, mode (i, j):   f_ij = (pi/2) * sqrt(D/m) * ((i/a)^2 + (j/b)^2)
```

Loss enters as a complex stiffness `E*(1 + j*eta)`, with `eta` the material's loss factor. Each edge is free, simply supported, or clamped. Free edges are the usual DML case and have no closed form, so the plate's modes are found numerically (the Rayleigh-Ritz method, below) and summed.

The exciter is an inertial motor. Its coil is fixed to the panel and its magnet hangs on a suspension. With `v_p` the panel velocity at the exciter and `v_m` the magnet velocity:

```
V       = Ze*I + bl*(v_p - v_m)
F_panel = bl*I + Zs*(v_m - v_p)                 Zs = k_s/(j*omega) + r_s, the exciter's suspension
magnet:   j*omega*m_mag*v_m = -bl*I - Zs*(v_m - v_p)
```

The coil's mass is added to the panel at the exciter position.

- **Backing.** `open`: both faces radiate. `sealed`: only the front face radiates, and the trapped air is a spring that puts a uniform pressure `-(RHO0*C0^2 / V_cavity) * integral of w over the panel` on the panel. A panel that opens into a cabinet chamber comes at Level 4.
- **Radiation.** Each panel element is a surface source with velocity `j*omega*w`. An `open` panel also has a back-face source at each element with the opposite sign.
- **Limits.** The coil's travel against the magnet (the gap an exciter's published travel describes) against the exciter's travel, voice-coil heating, and amplifier voltage.
- **Data.** The exciter comes from the catalog (section 9). The panel's `E`, `rho`, `nu`, and loss factor come from a source or a measurement. Printed panels vary with infill and print direction, so the interface says the result depends on those numbers.

```jsonc
{ "kind": "dml_panel", "exciter_id": "...", "mount": "m2", "backing": "sealed", "cavity_depth_m": 0.02,
  "panel": { "width_m": 0.30, "height_m": 0.20, "thickness_m": 0.003, "material": "...", "edges": "free" },   // edges: free | supported | clamped
  "exciter_at_m": [0.12, 0.09] }   // from the panel's lower-left corner, as seen from outside
```

The mount point is the middle of the panel, and the panel lies flat on the mount's surface with its width along the surface's `u`. `cavity_depth_m` is the depth of the air sealed behind a `sealed` panel; the cavity is as wide and as high as the panel. Nothing is cut into the cabinet for a panel: the wall behind it stays whole and is the back of that cavity. The panel's frame is not in the print file or the cost yet, and the geometry report says so.

As built:

- **Modes.** Rayleigh-Ritz on the square `[-1, 1]^2` with the basis `(1 - xi^2)^q * P_i(xi)` along each side: normalized Legendre polynomials times a factor that meets the edge condition, `q = 0` for free, 1 for simply supported, 2 for clamped. All four edges take the same condition. The strain energy is `(D/2) * integral of w_xx^2 + w_yy^2 + 2*nu*w_xx*w_yy + 2*(1 - nu)*w_xy^2`. The four symmetry classes (even or odd along each side) are solved apart, with Gauss-Legendre integration, a Cholesky reduction, and a Jacobi eigenvalue solver, all in the engine with no new dependency. The polynomial degree is 28 along the longer side.
- **How far up it reaches.** A polynomial basis resolves only so many half-waves along a side: `0.6 * (count + q) - 1.2`, with `count` the functions of one parity. Modes above the lowest frequency at which either side runs out are dropped, the fidelity label gives that frequency ("The panel's N resonances up to F Hz are modeled; above that its response is not, and reads low"), and results above it should not be read. For the `rear-panel` example (0.30 x 0.20 m, 3 mm) that is 144 resonances up to 6.3 kHz. A solved panel is kept, so editing anything else does not solve it again.
- **Exciter, coil, and cavity, in closed form.** With `G_n = 1 / (lambda_n*(1 + j*eta) - omega^2)` for each mode of unit modal mass, `phi_e` the mode's shape at the exciter and `s` its integral over the panel:
  ```
  g_ee = sum phi_e^2 * G     g_es = sum phi_e * s * G     g_ss = sum s^2 * G
  alpha = g_ee - kappa * g_es^2 / (1 + kappa * g_ss)       deflection at the exciter per newton; kappa = RHO0*C0^2 / V_cavity
  Y_p   = j*omega*alpha / (1 + j*omega*m_coil * j*omega*alpha)   the panel's mobility with the coil's mass on it
  Y_t   = Y_p + 1 / (j*omega*m_magnet)                     coil against magnet
  Zin   = Ze + bl^2 * Y_t / (1 + Zs*Y_t)
  F     = bl*I / (1 + Zs*Y_t)                              force on the panel and coil
  ```
  These are the three equations above, solved. The cavity is the one air spring of **Backing**.
- **Sources.** The panel is cut into elements no larger than 12.5 mm (at most 40 along a side), each a surface source with its own volume velocity. An element that falls outside the mounting surface is left out, with a warning.
- **It does not push on the cabinet's air.** A sealed panel has its own cavity, so it takes no part in the cabinet's enclosure model.

What validated it (CHK-PLATE-1 and the tests beside it):

- Simply supported `test-panel-a`: every resolved resonance against the closed form, 96 of them up to 2.8 kHz at degree 24 and 183 up to 5.2 kHz at degree 32, worst error 0.13 %.
- A free square plate, `nu = 0.3`: frequency parameters 13.468, 19.596, 24.270, 34.801, 61.093 against Leissa's tabulated 13.47, 19.60, 24.27, 34.80, 61.09. Clamped: 35.985, 73.394, 108.217 against 35.99, 73.41, 108.27.
- The exciter on a large panel sees, on average over frequency, the mobility of an endless plate, `1 / (8*sqrt(D*m))`: the resistance the panel adds to the coil reads 0.187 ohm against 0.195 ohm.
- A panel alone on a large flat surface radiates, from 700 Hz to 2.8 kHz, 0.15 dB below a point-driven endless plate, `W = RHO0 * F^2 / (2*pi*C0*m^2)`.

**Layered and direction-dependent panels** (as built, Level 3; owner, 2026-11-12: "include all novel materials that have promise for a cheap HiFi grade speaker"). A panel is one material at the panel's thickness, or `layers` listed from its back face to its front:

```jsonc
"panel": { "width_m": 0.30, "height_m": 0.20, "edges": "free",
           "layers": [ { "material": "pla-printed", "thickness_m": 0.0004 },
                       { "material": "thermhex-pp-honeycomb", "thickness_m": 0.005 },
                       { "material": "pla-printed", "thickness_m": 0.0004 } ] }   // a layer may take "turn_deg": 90
```

- **Stiffness.** Classical laminate theory: each layer is orthotropic in the panel's plane, its strong direction along `u` (or `v` when turned 90 degrees), `Q11 = E_u/(1 - nu_uv*nu_vu)`, `Q22 = E_v/(1 - nu_uv*nu_vu)`, `Q12 = nu_uv*Q22`, `Q66 = G_uv`, and `D_ij = sum of Q_ij * (z_top^3 - z_bottom^3)/3` about the stack's middle. The plate's strain energy is `(1/2) * integral of D11*w_xx^2 + 2*D12*w_xx*w_yy + D22*w_yy^2 + 4*D66*w_xy^2`, which for one isotropic layer is the formula above. Bending and stretching are uncoupled, exact for a stack that mirrors about its middle; another stack gets a label line.
- **Shear through the thickness.** `S = (5/6)*G_t*h` for one layer, `S = d^2 / sum(t_k/G_t,k)` for several (`d` between the middles of the outer layers; for a sandwich, `G_core*d^2/t_core`). Each mode is lowered to `omega^2 = omega_bend^2 / (1 + D*k^2/S)`, with `D = sqrt(D11*D22)` and `k^2 = omega_bend*sqrt(m/D)`: exact for a simply supported Mindlin plate without rotary inertia, an estimate for free edges. A soft core matters: 0.4 mm PLA skins on 5 mm of XPS lower the first resonance of a 0.40 x 0.30 m panel by 10 % and mode (8, 8) by 75 %.
- **Damping.** Each layer's loss factor weighted by its share of the bending stiffness. That leaves out the extra damping a soft core adds by shearing between stiff skins, and the label says so.
- **Missing figures.** A record without a loss factor takes 0.02 and one without a Poisson's ratio 0.3, both lab assumptions named in the label; a record without an in-plane shear stiffness takes `sqrt(E_u*E_v)/(2*(1 + nu))`. A core (`core_only`) has no stiffness in its own plane and needs `transverse_shear_pa`; it cannot be a panel alone.
- **Checks.** CHK-PLATE-2: an orthotropic plate with the Wood Handbook's balsa ratios, simply supported, against the closed form, 33 resonances up to 1.18 kHz within 0.05 %. One layer reproduces `D`, `nu*D`, and `(1 - nu)*D/2` exactly; a sandwich, a quarter turn, and three cross plies match hand calculations to 1e-12.

Not modeled: the air's load on the panel (it moves as in a vacuum: the air's added mass and the damping from sending out sound are left out, so resonances read a little high in frequency and too sharp), an open back, different conditions on different edges, anything above the modal limit, the exciter's own footprint (it pushes at a point), and the frame.

### 5.10 Room estimate

`free` is no room: only the direct sound of section 5.6. It is the default and the only member through Level 2.

`simple`, Level 3, adds the two things a radiator needs when it does not face the listener: the wall behind the speaker, and the room's diffuse sound.

```
"room": { "kind": "simple", "wall_behind_m": 0.5, "wall_reflection": 0.9, "volume_m3": 50, "rt60_s": 0.4 }

direct       the direct sound of section 5.6
reflection   the cabinet mirrored in the wall behind it: the sound it sends toward that wall,
             arriving later by the extra path and scaled by wall_reflection
diffuse      p_rev^2 = 4 * W * RHO0*C0 / A,   A = 0.161 * volume_m3 / rt60_s,   W = radiated sound power

in-room response: direct and reflection add as complex pressures; diffuse adds as energy
```

The wall is `wall_behind_m` behind the cabinet's back. The cabinet mirrored in that wall is the same as the listening position mirrored in it, heard from the real cabinet, so the reflection is the radiation model's pressure at that mirrored point, which is behind the cabinet, times `wall_reflection`. It ignores the cabinet standing in the reflection's way back. `W` is the sound power of the whole speaker as it is driven, with every radiator's DSP chain and polarity.

The results keep the direct sound as it is (`summed_spl_db`, and the complex pressure listen mode uses) and add `in_room`: the in-room level, the early sound (direct plus reflection), the reflection alone, and the diffuse level. A design with a room is scored on the in-room level. The reflection and the diffuse sound use the piston model, without the drivers' published response curves.

This is an estimate for comparing designs. It is not a room simulation.

### 5.11 Wall vibration

Cabinet walls are not rigid. The pressure inside pushes every wall, the wall bends, and it sends out sound of its own. A request with `walls` models it; the page, scores, the optimizer, and the generator ask for it, and the engine's default, which the numbered checks use, does not. It is what makes a wall build (section 6.2) heard: sand-filled, plaster-filled, and two-filament walls change the sound, not just the cost.

**Each flat wall is a plate**, simply supported along its edges, spanning between the middles of the walls around it (the outline less one wall thickness each way). Its layers come from `build.material` and `build.wall`, from the inside face out:

```
Q_k        = E_k*(1 + j*eta_k) / (1 - nu_k^2)            each layer, with its loss
D_local    = sum Q_k * t_k^3 / 12                        layers bending about their own middles
D_sandwich = sum Q_k * t_k * z_k^2                       layers carried apart by the core
S          = d^2 / sum(t_k / G_k)                        shear through the thickness (G complex)
D(k)       = D_local + D_sandwich / (1 + D_sandwich*k^2 / S)
```

`D(k)` is the sandwich result of Mead and Markus, the Ross-Kerwin-Ungar model for a symmetric stack. A soft lossy core between stiff skins shears as the wall bends and damps it, with loss factor `beta*Y*g / (1 + (2 + Y)*g + (1 + Y)*(1 + beta^2)*g^2)`, `Y = D_sandwich/D_local`, `g = S/(D_sandwich*k^2)`, `beta` the core's loss factor (CHK-WALL-2). A stiff bonded core (a gypsum cast, a stiff filament) gives the laminate's own stiffness. A loose fill (sand) carries no stiffness and no shear, so the skins bend apart and the fill adds mass; its loss enters as its own loss factor times its share of the mass, since at a resonance the energy it holds is its share of the kinetic energy. A solid wall is one layer of the print material. A material whose stiffness is not published is a loose mass, and a missing loss factor or Poisson's ratio takes the lab's 0.02 or 0.3, each named in the label.

**Modes.** The wall's modes are the supported plate's (section 5.9's Rayleigh-Ritz solver, degree 16), solved once per wall size with unit stiffness and mass, since a supported plate's shapes do not depend on them; each mode then takes the stack's complex stiffness at its own wavenumber, `k^2 = sqrt(lambda*m / D(0))`:

```
lambda_n = lambda_ref,n * D(k_n) / D(0) * (1 + j*eta_fill)
s_n      = integral of mode n's shape over the wall          (unit modal mass)
Y_wall   = j*omega * sum s_n^2 / (lambda_n - omega^2)         outward volume velocity per pascal inside
```

**Joined to the cabinet.** In a cabinet with one air space, all walls together are a shunt on the driver's chamber: `Z_ab' = Z_ab / (1 + Z_ab*Y_walls)`. Every flow in the cabinet follows that chamber's pressure, so each port's `H_k` scales by the same factor, and this holds for every enclosure model, the full 3D one included (its grid's walls stay rigid; the shunt is applied to its result). Each wall is one surface source at its middle, the size of the wall (section 5.6) with its net volume velocity `Y_wall * Z_ab' * U_in`, `U_in = -U_d`, (3 by 3 patches per wall moved the largest difference from rigid walls by at most 0.2 dB on `sealed-test`, at five times the cost on the page's grid), so the walls are heard through the same radiation model, room, listening window, sound power, and DSP chain as the driver that pushes them. Below its resonances a wall is a spring: the box acts larger, and the wall gives back, in opposite phase, the share of the cone's flow it took. Near a resonance it sings; above, its mass holds it still.

**Cabinets divided into chambers** (as built, 2026-11-13; partitions 2026-11-15). Each side of a chamber that lies against the cabinet's outer wall (the plastic between it and the outside air is at most a wall and a half thick, at its middle and four points halfway to its corners) is a plate over that side, spanning the side plus one wall; sides against thicker plastic are held still. A face against another chamber is a **partition**: a solid printed plate of the gap's thickness, simply supported over the overlap of the two faces, pushed by the pressure difference `(P_a - P_b)`. It couples the air network as a mutual admittance (`U = Y (P_a - P_b)`) and does not radiate outside. Each chamber's outer walls are a shunt on that chamber: `j*omega*V/(RHO0*C0^2) + Y_walls` in the quick model and the detailed model's chamber node alike. When partitions vibrate, the nodal matrix (quick network) carries them; the detailed tree cannot, so those curves keep the quick network with a label line. With the full 3D model, whose grid's walls are rigid, the driver's chamber's outer walls are added to its result as a shunt, and the other chambers' pressures come from the detailed model; the label says so.

**Stock cabinets** (as built, 2026-11-13; MDF/plywood 2026-11-15). A `sheet_box`'s six panels are plates of its sheet, from the stock record's stiffness and density (a published range taken at its middle, with a label line); a sheet whose record lacks either keeps rigid walls, with the reason. Of the catalog's sheets, foam board, MDF, and plywood all publish enough to vibrate. A `tube`'s two printed end caps vibrate, each a plate of the print material and wall build at the cap's thickness, taken as a square of the pipe's inside area; the curved pipe is held still.

**Imported cabinets** (as built, 2026-11-15). A `mesh` cabinet's flat, axis-aligned outer walls vibrate: the air's bounds are found from the solid, each face's thickness is measured through the plastic, and the plate uses the design's print material and wall build (sand-filled and two-filament included). Faces against another enclosed air space are partitions, as for printed divided cabinets. Curved or slanted skin is held still. Per-part materials are not yet used for the wall stack (the design's `build.material` and `build.wall` are).

As built, checked by CHK-WALL-1 and CHK-WALL-2 and a test beside them: a supported PLA wall under uniform pressure against Navier's series, every driven resonance within 0.1 % (1 % near the basis limit) and the response within 1 % at, between, and below them; the static compliance within 1 % of the whole series; the stack's stiffness against a hand calculation to 1e-9 and against the Ross-Kerwin-Ungar loss factor within the core's own bending loss; and, through the simulator, `sealed-test` at 10 and 20 Hz against a rigid box larger by the walls' compliance plus their give-back, within 0.01 dB. The same identity holds within 0.01 dB for two sealed chambers with a driver each (each chamber larger by its own walls' compliance), a PVC tube's end caps, and a foam-board sheet box (at a thirtieth of its 88 Hz wall resonance, where its walls hold 4.4 times the air's compliance); a chamber that fills the whole inside gives the undivided cabinet's result within 1e-13 dB.

**Braces** (as built, 2026-11-15). A `brace` solid that reaches a vibrating wall face cuts that plate into smaller simply-supported rectangles through the contact (a shelf or stud across the plate, or the end of a bar into the plate), so the wall's resonances rise; the brace also takes its share of the air and prints. Braces that do not reach a vibrating face still take air and print.

Not modeled, each said in the label: the air's load on the walls; the pattern of standing waves' pressure on them (each wall feels the chamber's mean pressure, which holds while the chamber is small against a wavelength); the driver's frame shaking the baffle; holes in the walls; curved skin (a `revolved` body's flat ends vibrate, its curved side does not, nor does a tube's pipe, nor slanted imported faces); a wall that carries a panel speaker, which is held still; walls not rectangles, taken as rectangles of the same area; per-part materials on an imported assembly (the design's wall build is used instead). Each mode takes its stiffness at its own wavenumber, an approximation for a frequency-dependent core away from that mode's resonance.

## 6. Cabinet builder

### 6.1 What it is

A parametric model of the cabinet. The user edits numbers and drags handles, and the solid is regenerated. The lab also generates geometry itself: whole cabinets (section 11.4) and plastic placed inside by the 3D model (section 5.5). What it generates is written into the same design file, as the same kinds of parts. The user does not sculpt by hand.

### 6.2 Geometry data

**Outer shape.** The cabinet's outside is a member of the outer-shape family (section 3.3). `box` is the Level 1 member. Level 2 adds rounded and chamfered edges, a sloped baffle, and `prism`, which is any closed outline extruded along one axis. Level 3 adds `revolved` shapes such as cylinders, spheres, and eggs, and `loft`, which places cross-sections along a spine. An imported assembly, `mesh`, was brought forward to Level 3 at the owner's request (below). The wall follows the outer shape at `wall_m` thickness unless the chambers say otherwise.

The Level 2 printed shapes, as they appear in a design file:

```jsonc
{ "kind": "rounded",   "width_m": 0.16, "height_m": 0.26, "depth_m": 0.20, "wall_m": 0.008, "edge_radius_m": 0.02 }  // all 12 edges
{ "kind": "chamfered", "width_m": 0.16, "height_m": 0.26, "depth_m": 0.20, "wall_m": 0.008, "chamfer_m": 0.015 }    // 45-degree, all 12 edges
{ "kind": "sloped", "width_m": 0.16, "height_m": 0.26, "depth_bottom_m": 0.22, "depth_top_m": 0.16, "wall_m": 0.008 } // the front leans back; the back is vertical
{ "kind": "prism", "outline_m": [[0.0, 0.0], [0.16, 0.0], [0.08, 0.2]], "length_m": 0.2, "wall_m": 0.008 }       // the front's [x, z] corners, extruded back along y
```

The Level 3 curved shape, `revolved`: a body of revolution about the front-to-back axis, with a flat round front for a driver and a flat round back.

```jsonc
{ "kind": "revolved", "diameter_m": 0.24, "length_m": 0.28, "front_diameter_m": 0.16, "back_diameter_m": 0.08,
  "widest_at": 0.4, "wall_m": 0.008 }   // an egg; equal diameters make a cylinder, zero flats and widest_at 0.5 a spheroid
```

The outside radius runs from the front's rim to the widest point, `widest_at` of the way back, along a quarter ellipse, and from there to the back's rim along another. The inside is every point at least `wall_m` from the outer surface, found numerically as a profile of 96 steps; volumes are those of the stepped profiles, and the print solid revolves the same profiles in 96 segments. Its surfaces are `front` and `back`; `[u, v]` is measured from the lower-left corner of the square the body fits in, so the middle of a flat face is `[diameter/2, diameter/2]`, and a mount must sit on the flat. The everyday radiation model takes the flat front's rim as the baffle edge, though the skin curves away gently there, and the cabinet's notes say so. `loft` is not built.

**Imported speakers, `mesh`** (as built, Level 3; owner, 2026-11-12: import a whole speaker with internal baffles and damping, give each part a material, place drivers in any orientation). The page reads a 3MF file (one part per object the build places, with its units, components, and transforms) or STL files (one part each, in millimeters). STEP is not read yet; it can be added later as another importer that produces the same parts.

```jsonc
{ "kind": "mesh",
  "parts": [ { "name": "shell", "role": "solid", "material": "pla",
               "vertices_m": "<base64 of little-endian f32 x, y, z>", "triangles": "<base64 of little-endian u32 a, b, c>" },
             { "name": "wool", "role": "damping", "material": "...", "flow_resistivity_pa_s_m2": 10000, ... } ],
  "points": [ { "id": "at-m1", "point_m": [0.08, 0.0, 0.15], "normal": [0.0, -1.0, 0.0],
                "base_normal": [0.0, -1.0, 0.0], "tilt_deg": 0, "turn_deg": 0 } ] }
```

- Every part must be a closed mesh; an open one is refused with its name. Space is sorted on a grid of columns along `z` (no finer than 2 mm, at most about 4 million cells): each column's crossings with each part give that part's inside exactly along the column. A grid cell is solid when at least half of 32 samples in it are (16 columns a quarter of a cell apart, at two heights), so a glued seam between printed parts, thinner than half a cell, stays closed. Air that cannot reach the outside without passing through a solid part is the cabinet's air; a damping part is air the sound passes through. Volumes integrate the exact crossings over columns a quarter of a cell apart.
- A part's `role` is `solid` (printed, in its own `material`, priced by it) or `damping` (listed in litres; its flow resistivity is the part's own figure, or its material's record in `catalog/damping/`).
- Mounting points are the surfaces: a mount's `surface` names a point, and `at_m` is ignored. The page places a point where the user clicks the model, facing out from the surface there; `tilt_deg` and `turn_deg` turn it from that face (`base_normal`), and `normal` is the result. The surface's outline for edge diffraction is the flat region around the point that shares its plane; the wall thickness is found by a ray through the part. A port placed on the model is a 40 mm round duct 100 mm long, straight inward.
- The print tree holds the parts as meshes (op `mesh`), less the holes and ports. A mount not yet placed is a warning, not an error: the speaker is shown and the print file waits.
- **A model that already has its holes** (as built, Level 3; owner, 2026-11-13: import a known speaker exactly, from its designer's files). A point may mark a hole in the model itself: `"hole": "seal"` for a hole that what is mounted there fills (a driver's cut-out and recess, a wire hole), `"hole": "opening"` for the mouth of a duct moulded into the model. The hole is the open area under the point on the first layer of grid cells below the surface (it must close within 12 cm of the point, on a face that faces along one of the model's axes). While the outside floods in, every marked hole is held closed; afterwards a seal stays solid through the wall, layer by layer until the hole widens by a quarter past its narrowest layer (at most 30 layers), and a mouth is air again.
- With holes marked, each enclosed air space is a chamber of its own, `space-1`, `space-2`, ... from the largest, and a chamber exists for each space a sound source or a moulded duct opens into; a design with marked holes lists no `chambers`. Each sound source pushes into the space behind its point, whatever its mount's `rear_into` says.
- **A moulded duct is measured from its air.** Every cell of its space gets its distance from the mouth along the shortest way through the air (26 neighbours); each cell of distance is one slice of the duct, its size the cells in it. The duct ends at the first slice more than 2.5 times the median of those before it, where it widens into a chamber. Its center line runs through the slices' middles (smoothed over three), and the slices' areas are scaled so the duct holds its exact air: its own cells' and its share of the part-solid cells along its walls, integrated on the fine columns. Its air comes out of its chamber. The quick model takes it as one air mass, `integral of dx / S`, with the lab's usual end corrections (inside end flush, mouth free); the detailed model chains its slices, merged to about 10 mm; the full 3D model has the model's own bore, and its opening is the mouth's area two cells deep. Its mouth radiates like any port's. Checked on a box with a 60 x 10 mm slot 100 mm long moulded in: 101 mm and 6.02 cm^2 measured, and the same tuning as the lab's own slot port to 0.01 Hz in the detailed model and 1.4 % in the quick one. A bend counts as straight, as for any duct.
- On the page, "The model already has this hole" marks a placed sound source as a seal; "Mark a sealed hole" and "Mark a moulded port mouth" place free marks for wire holes and moulded ducts. Each mark can be changed or removed after placing.

The wall follows the outside everywhere: the inside of a rounded box is rounded by `edge_radius_m - wall_m`, a chamfer's inside leg is `chamfer_m - wall_m * (2 - sqrt 2)`, and a prism's inside outline is every side moved inward by `wall_m`. Rounded and chamfered boxes keep the six box faces, and a mount must sit on the flat part of its face. A sloped box's faces are `front` (the slope), `back`, `top`, `bottom`, `left`, and `right`. A prism's faces are `front`, `back`, and `side-1`, `side-2`, ... for the sides from the first outline corner onward. On any face, `[u, v]` is measured from the lower-left corner as seen from outside, with `v` running up the face; the top and bottom keep the box's convention.

**Build method.** Each part of the cabinet is either printed or a stock part from `catalog/stock/`, such as a length of PVC pipe or a cut sheet. A hybrid build combines them: for example, a stock pipe for the body with printed end caps, baffle, and port. The outer-shape member `tube` (Level 2) is a stock pipe or tube cut to length, whose diameter and wall come from its stock record. The member `sheet_box` (Level 2) is a box cut from sheet stock such as MDF, plywood, or foam board, with a cut list. Printed parts and stock parts both feed the air network, the voxel grid, the cost, and the export: print files for printed parts, and a cut list for stock parts.

**Wall builds** (as built, Level 3; owner, 2026-11-12). A printed cabinet's walls can be two thin skins with a core between them, in `build.wall`:

```jsonc
"wall": { "kind": "filled", "skin_m": 0.0016, "fill": "quikrete-play-sand" }          // poured: catalog/fills/
"wall": { "kind": "two_material", "skin_m": 0.0016, "core": "varioshore_tpu" }       // a second filament
```

A filled wall is printed hollow and filled with sand or gypsum cement (the "sand-filled" and "dental plaster" DIY builds); a two-material wall prints its core in a second filament, soft or foaming, which damps the wall the way a rubber layer between two sheets does (constrained-layer damping). The core's volume is estimated as the print's solid times `1 - 2*skin_m/wall_m`; the cost prices it from the fill's `buy_kg_per_m3` or the filament's printed density, and a printed core counts toward the plastic and the print time. A wall build is heard through section 5.11's wall model when the request asks for walls (the page does); a request with rigid walls says so (`wall_build_not_heard`). A fill or core with a published stiffness is bonded to the skins; one without is a loose mass between them. Sand is loose, so a sand-filled wall is only as stiff as its two skins: thin skins make it heavy and soft.

Printing is optional. A cabinet can be all stock, all printed, or a mix, and the lab prints a part only when that improves the sound or lowers the cost. When the same cabinet can be built more than one way, the lab shows the ways side by side, each with its cost and its results. Printing earns its place through geometry that stock parts cannot make, such as shaped ports, folds, and curved baffles. Wall stiffness also differs between materials; section 5.11 models it for printed walls, tube end caps, and sheets whose records publish stiffness and density; other stock walls are still rigid, which the comparison says.

**Mounts.** A mount names one of the outer shape's surfaces and a position on it. The outer shape returns the mount's position and outward direction, so a radiator can sit on any surface and face any way: front, back, side, top, or along a curve.

**Inside.** Section 4.1 shows the Level 1 enclosure. From Level 2 the same fields describe ports, extra chambers, and internal ducts:

```jsonc
"chambers": [ { "id": "main", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242], "fill": null } ],
"ducts": [ { "id": "port", "from": "main", "to": "outside",
             "path_m": [[0.080, 0.060, 0.050], [0.080, 0.0, 0.050]],   // center line; a corner in the path is a bend
             "section": { "shape": "round", "start_m": [0.035], "end_m": [0.035] } } ],   // round: [diameter]; rect: [width, height]; start != end is a taper
"solids": [ { "id": "brace-1", "kind": "brace", "min_m": [0.008, 0.080, 0.120], "max_m": [0.152, 0.100, 0.130] } ]   // a bar across the chamber
```

Placed plastic is a solid of kind `voxels`: plastic cell by cell, which is what "Shape the inside" writes (section 5.5).

```jsonc
{ "id": "shaped", "kind": "voxels", "min_m": [0.0, 0.03, 0.03], "dx_m": 0.01, "size": [16, 15, 24],
  "cells": "12 3 40 5" }   // run lengths in the order (i*ny + j)*nz + k: air, plastic, air, ... starting with air
```

Only the part of a cell that lies in the cabinet's air counts, so cells may reach into a wall; that closes any gap between the plastic and the wall it hangs on. The plastic takes its volume from the air of the quick and detailed models (from its chamber, when chambers are listed), is solid in the 3D grid, and is printed as merged boxes cut to the air. Its volume is counted from 6 x 6 x 6 points per cell. A `brace` is a rectangular bar (`min_m` / `max_m`): it takes air, prints, and fills the 3D grid the same way, and each wall face it reaches is split into smaller supported plates (section 5.11). A solid of any other kind is left out, with a warning that names it.

The air region is chambers plus ducts minus solids. Chambers are the nodes and ducts are the edges of the air network that `lumped` and `waveguide` solve. From Level 3 a driver may mount directly on a duct, as in a transmission line.

A chamber is a box of air inside the cabinet's inside air; chambers may not overlap. When chambers are listed, the inside air no chamber claims is printed solid: the gaps between chambers are the partition walls. Each mount's `rear_into` and each duct's `from` and `to` name a chamber (or `outside`), and a driver's hole must open into the chamber it names. A printed duct's path is straight runs joined at corners. Each run is lengthened past a corner by half the section in the plane of the turn, so the runs meet without a notch. A round duct bends any way; a slot (`rect`) bends only within one plane, with its width across the fold and its height in the plane of the bends. Either may taper. A port's last run leaves its face square and is bored through it, a duct between chambers is bored through the partition, and where a duct runs through chamber air it is printed as a tube (or a slot) with a 3 mm wall unless the duct gives `wall_m`. Where it runs through the solid that unclaimed inside air becomes, it is simply bored: that is how a folded line is printed. The tube's outside takes air from the chamber. Plastic and air are counted along the center line, which is exact for a slot's square corners and close for a round bend. Several chambers are solved by `lumped`, each chamber one compliance and each duct one air mass, and by `waveguide` as a tree (section 5.4).

### 6.3 Three views of one solid

From the same parameters derive:

1. The air network (volumes, lengths, areas) for `lumped` and `waveguide`, computed by formula in the engine.
2. The print solid, built on the web side with constructive solid geometry using the `manifold-3d` package, the Manifold kernel compiled to WebAssembly. It guarantees manifold output. Run it in a worker. Call its `setup()` after loading, and release every object it creates with `.delete()`, because they are not garbage collected. Cost uses the engine's own solid volume, not this solid's.
3. The voxel grid for `fdtd`, sampled in the engine directly from the primitives, or from the print solid's mesh for an outer shape that has no formula.

These must agree (CHK-GEO-1). When they do not, the geometry code is wrong.

### 6.4 Export

- Binary STL from Level 1 and 3MF from Level 2. Print files are in millimeters, so multiply by 1000 on export.
- Solids are watertight and manifold (CHK-GEO-2).
- Driver cut-outs, recesses, and bolt holes come from the driver's mounting data. When a record gives no bolt circle, the hole is still cut and the screw holes are left out, with a note to mark them through the driver's flange and drill them (owner, 2026-11-12: a missing detail must not stop a design). A missing cut-out size still refuses the print file.
- From Level 2, split the cabinet into parts that fit the printer's build volume, add alignment features, and report part count, material mass, and print time. The engine picks the number of pieces along each axis for whichever way the cabinet can stand on the bed gives the fewest parts, cuts evenly, and places up to three alignment pin holes (4.2 mm, 8 mm deep each side) across every joint where there is plastic all around them, away from driver flanges and ports. The pins are printed (4 mm by 15 mm) and counted in the plastic. The page cuts the solid into the parts with manifold-3d and exports one 3MF object per part and per pin.
- An OpenSCAD program (`cabinet.scad`), from Level 3: the engine's own recipe of solids written out in millimeters (cubes, cylinders, polyhedra for hulls, extruded outlines, unions and differences), with the print cuts as a `part` setting and the pin as a module. OpenSCAD opens it, and FreeCAD through its OpenSCAD workbench. It goes one way: a file changed there does not come back into the lab.
- Enforce a minimum wall thickness and warn about thin features. Printed walls vibrate in the simulation (section 5.11), as do sheets with published stiffness and density; other stock walls do not yet, and the interface says so.

## 7. Crossover and DSP chain

- Order for each radiator: crossover filters, per-radiator EQ, delay, gain and polarity, limiter. Blocks for the whole speaker come before the split.
- Biquads follow the RBJ Audio EQ Cookbook, with `H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)`. Types: lowpass, highpass, peaking, low shelf, high shelf, allpass, notch.
- Crossover families: Butterworth of order n, and Linkwitz–Riley of order 2n, which is two Butterworth filters of order n in cascade. LR4 and LR8 sum flat in phase. LR2 needs one side inverted. The lab sets polarity automatically and shows the summed response either way.
- Every block provides its complex frequency response for simulation and a sample-by-sample process function for audio, from one implementation.
- Delay is whole samples at Level 1 and fractional from Level 2. FIR blocks and linear-phase crossovers come at Level 3, with the added latency shown.
- Changing a parameter during playback crossfades between old and new settings over about 20 ms.
- Export for real hardware: for each radiator, a text file of biquad coefficients at a chosen sample rate, and FIR filters as mono WAV files. Hardware-specific formats wait for the owner's choice of playback hardware (`STATUS.md`).

**Passive crossover** (as built, 2026-11-15). A design may set `dsp.passive` to an R/L/C network from one amplifier to the drivers, the way a kit crossover board works. At each frequency the lab solves the network with each driver's own terminal impedance (coil plus the box load) as the load, and that complex voltage drives the radiator in place of a digital crossover block. Gain, delay, and polarity on each radiator still apply. Digital crossover or EQ blocks listed on the radiators are left out of the graphs when `passive` is set, with a label line. Kit check: `designs/imports/c-note/` (Parts Express C-Note MT) against ASR CTA-2034 curves in `asr/` (CC BY-NC-SA). Listen mode folds each radiator's network transfer into that radiator's listen impulse (amplifier to the listening position), with driver loads frozen at the simulator's impedance at each frequency; the worklet does not play the leftover digital radiator blocks. The page edits the netlist (add, change, or remove R, L, C, and driver loads; values in ohm, mH, and µF). **Optimizing R/L/C values** (as built): when `dsp.passive` is set, "Tune the passive board" on the cost panel searches those part values toward the quality target with Nelder–Mead (cabinet, drivers, and wiring stay fixed; digital correction is not applied, because those filters are left out under a passive board). Each R, L, and C may move by up to a factor of four from its starting value, within absolute bands. The search repeats from its seed.

```jsonc
"passive": {
  "source": ["amp", "gnd"],
  "elements": [
    { "kind": "L", "a": "amp", "b": "w", "henry": 0.0015 },
    { "kind": "C", "a": "w", "b": "gnd", "farad": 10e-6 },
    { "kind": "driver", "a": "w", "b": "gnd", "radiator": 0 },
    { "kind": "C", "a": "amp", "b": "t", "farad": 3.3e-6 },
    { "kind": "driver", "a": "t", "b": "gnd", "radiator": 1 }
  ]
}
```

Kinds: `R` (`ohm`), `L` (`henry`), `C` (`farad`), and `driver` (`radiator` index into `speaker.radiators`). A driver load may sit between any two nodes, so two woofers can be wired in series (one current through both, as in a dual-woofer tower) or each driver can have its own branch. Checked against a series resistor into an equal load (half voltage), two equal drivers in series (half voltage each, amplifier sees the sum), and against first-order LR low-pass and CR high-pass on a resistive load at the −3 dB frequency.

## 8. Correction and auto EQ

Purpose: get the most out of cheap hardware and never ask it for more than it can do. Inputs are the simulated response per radiator and summed (later a measured one), the limits of section 5.7, and the target.

| | Feature | What it does | Level |
| --- | --- | --- | --- |
| C1 | Target fit | Parametric EQ that brings the response to the target curve. | 1 |
| C2 | Physics-limited bass extension | Extends the bass by boosting only as far as cone travel, amplifier, heating, and port limits allow at the design level, with a protective high-pass below. | 1 |
| C3 | Geometry-derived compensation | Baffle-step and edge-diffraction correction computed from the cabinet's outer shape. | 2 |
| C4 | Time and phase alignment | Per-radiator delay from the radiator positions, and phase alignment through the crossover. | 2 |
| C5 | Cabinet-resonance correction | Finds internal resonances in the `fdtd` result that reach the listener through the cone or a port, cuts them, and tells the user which geometry change would remove the cause. | 3 |
| C6 | Level-aware bass | Deep bass at normal volume that backs off as level rises: a look-ahead limiter driven by a running model of cone travel, `X/V = bl / (j*omega*(Ze*Zm + bl^2))`, realized as a digital filter. | 3 |
| C7 | Linear-phase correction | FIR correction and crossovers, with latency reported. | 3 |
| C8 | Pre-distortion, experimental | Cancels a driver's own distortion using a nonlinear driver model. The interface labels it unproven until a measurement confirms it. | 4 |
| C9 | Correction from measurement | The same tools fed by a measured response instead of a simulated one. | 4 |
| C10 | Indirect-radiator tuning | Sets level, band limits, and delay for radiators that do not face the listener, using the room estimate of section 5.10. | 3 |

C1 works this way, so that behavior is the same from session to session:

1. Smooth the response to 1/6 octave. The error is target minus response, in dB, over the correction band.
2. Add one peaking filter at a time at the largest remaining error region, taking center, gain, and Q from that region. Then refine all filters together by least squares. Stop at `max_filters`, or when a new filter improves the RMS error by less than 0.1 dB.
3. Gain per filter stays between -12 and +6 dB. A boost has Q of 4 or less. Cuts are always allowed.
4. Do not fill a narrow dip. A dip with Q above 4 in the uncorrected response is a cancellation, which EQ cannot fill. Report it as a crossover or geometry problem instead.
5. Do not boost inside the protected band, 3 kHz to 5 kHz by default. This is the owner's rule.
6. Total boost at any frequency stays within `headroom(f)`.
7. Keep the result only if it helps. If flatness improves by less than 0.2 dB, or any target metric gets worse, return no correction and say why.

C2 finds the lowest frequency `f_low` (at most 300 Hz) at which `max_spl(f)` still reaches the design level, shapes the low end into a clean high-pass at `f_low` (second order for a sealed box, fourth order for a vented one) by boosting within headroom, and always adds a protective high-pass below. Below a vented box's tuning the cone is unloaded, so that high-pass is not optional. When the speaker cannot play the design level even between 300 Hz and 3 kHz, C2 shapes the bass for the highest level it reaches there, and says so; when no shape fits at all, it keeps the natural bass (and a vented box's protective high-pass). Above 3 kHz, where bass filters change nothing, the coil and the amplifier set the limit and C2 does not judge it.

The target curve is flat from `f_low` to 20 kHz with that high-pass shape below, plus an optional tilt in dB per octave that defaults to 0.

Every filter in the output carries a one-sentence reason in plain language, for example "Cuts 3 dB at 1.1 kHz: peak from the baffle edge".

## 9. Catalog and cost

### 9.1 Driver records

One JSON file per driver in `catalog/drivers/`.

```jsonc
{
  "id": "test-woofer-a",
  "status": "synthetic",              // "sourced" for a real part
  "maker": "Test", "model": "Woofer A", "kind": "woofer",   // woofer | midrange | tweeter | fullrange
  "published": { "fs_hz": 50.0, "qes": 0.45, "qms": 4.0, "vas_m3": 0.010, "re_ohm": 6.0,
                 "le_h": 0.0005, "sd_m2": 0.0085, "xmax_m": 0.004, "pe_w": 30.0 },
  "mounting": { "cutout_diameter_m": 0.118, "outer_diameter_m": 0.146, "bolt_circle_m": 0.132,
                "bolt_count": 4, "depth_m": 0.060 },
  "response_file": null,              // Level 2: on-axis curve, with its measurement conditions
  "source": null,                     // sourced: { "url": "...", "title": "...", "read_on": "YYYY-MM-DD" }
  "price": null                       // sourced: { "amount": 0.0, "currency": "USD", "vendor": "...", "url": "...", "read_on": "YYYY-MM-DD" }
}
```

Rules:

- A `sourced` record is written with the manufacturer's data sheet open in the same session. Copy the numbers as printed and convert them to SI. Do not fill a field from memory, from a forum, or from another tool's database. If a field is not printed, leave it out and let the engine warn. Never write 0, or a sibling model's value, in its place: a missing size stays missing, and the builder refuses to cut a hole it does not know.
- If you cannot open the source, do not add the driver. Note the gap in `STATUS.md`.
- **Independent measurements** (owner, 2026-11-12). Makers' sheets are often optimistic, so a record may also carry what a respected independent tester measured on real samples: Voice Coil magazine's Test Bench, HiFiCompass, and similar labs with a published method. They go in an `independent` list beside `published`, never in place of it, each with who measured, the title and link, when it was published, and `read_on`; the source is open in the session, as for the maker's sheet. Its `values` hold any of the `published` fields in SI (`xmax_m` is the usable cone travel at the tester's distortion criterion, stated in `note`); `breakup` lists cone break-up peaks as `{ "f_hz", "peak_db" }`. The first entry's values replace the maker's in the simulation; the driver's note in the fidelity label names both, so every result says whose number it used. A driver with a measured break-up is warned about when its low-pass is not at least an octave below it, and the generator keeps its crossover there. Numbers read off a graph are not copied: only numbers the tester prints. When the independent entry prints a sensitivity that disagrees with Vas by more than 1.5 dB, Vas still sets compliance and the radiated level follows that independent sensitivity (owner 2026-11-15; section 5.1).
- **Measured response curves** (owner, 2026-11-12: download from respected HiFi community and industry sources). A curve file is copied from a published data file (a maker's FRD, or a tester's) without changes other than keeping every second point, and names its source, revision, and `read_on`. A data package that turns out to hold another part's data is not used.
- Every real record has a `tier`: `budget` or `reference`. Budget parts are the point of the lab: inexpensive drivers, full-range units, amplifier boards, and DSP options sold new today. Reference parts are a small set of well-regarded quality components, kept so that designs can be compared with a conventional build. When choosing what to add, budget parts come first.
- Only new, off-the-shelf parts with published data. Used, salvaged, or self-measured parts are not catalog records.
- `synthetic` records exist for checks and demos. Their ids start with `test-`. The interface shows them as "Test driver, not a real product". The optimizer never proposes them.
- Prices go stale. Show `read_on` beside every price, and warn when a price is more than 90 days old. Prices are in USD.
- A figure read anywhere other than the part's own page, such as a search result's extract or another product's listing, carries `"via"` naming where it came from (`"search_extract"` or `"other_listing"`). The interface shows it as unconfirmed, and a cost that includes it says so. Replace it, and drop `via`, when the part's own page opens.

Amplifiers and DSP devices in `catalog/electronics/` follow the same sourcing rules. An amplifier record holds channel count, power per channel at a rated load, and price. A DSP device record holds output channels, sample rates, filter capacity, and price. The synthetic amplifier `test-amp-a` has one channel of 50 W into 4 ohm.

**Novel materials** (owner, 2026-11-12). The catalog holds the cheap materials with promise for hi-fi parts, each from a source opened in the session:

- `catalog/panels/`: panel materials. Isotropic records give `youngs_pa`; direction-dependent ones give `youngs_u_pa`, `youngs_v_pa`, `poisson_uv`, and `shear_pa` (wood, from the Wood Handbook's elastic ratios); cores give `core_only` and `transverse_shear_pa`. Each gives `density_kg_m3`, and `poisson` and `loss_factor` when published (section 5.9 says what the lab assumes otherwise). Present: XPS foam board, two foamed PVC sheets, polycarbonate, balsa sheet, end-grain balsa, birch veneer for plywood, polypropylene honeycomb, PET foam, solid printed PLA, and foamed LW-PLA.
- `catalog/damping/`: fill for the inside of a cabinet, with `flow_resistivity_pa_s_m2` (a published minimum is stored as that minimum, and the note says so): mineral wool, glass wool, polyester wadding.
- `catalog/fills/`: what a hollow printed wall is filled with: `density_kg_m3` as placed, `buy_kg_per_m3` (powder bought per volume, for a cast), a price per kilogram, and for the wall model (section 5.11) `loss_factor` and, for a cast that sets bonded to the skins, `youngs_pa`, each with its own source: play sand, gypsum cement. A print material may give `loss_factor` too.
- `catalog/materials.json`: print materials, including foaming filaments (`"foaming": true`): their `density_kg_m3` is the printed density at the stated setting, and `filament_density_kg_m3` the filament's own. Present: colorFabb LW-PLA, Bambu PLA Aero, colorFabb varioShore TPU, Siraya Tech Roamr TPU Air.

A figure a source gives as a range is stored as the middle of the range, and one derived from a source (a foamed density from the flow setting, a wood density from its specific gravity) is marked as derived in the record's note.

Exciters, from Level 3, are records like drivers, in `catalog/exciters/`. They hold what an exciter's data sheet prints: coil resistance and inductance, force factor, coil and magnet mass, suspension stiffness and loss, travel, power, and mounting footprint (`published`: `re_ohm`, `le_h`, `bl_tm`, `coil_mass_kg`, `magnet_mass_kg`, `suspension_n_per_m`, `suspension_ns_per_m`, `xmax_m`, `pe_w`). Panel materials, also from Level 3, in `catalog/panels/`, hold `youngs_pa`, `density_kg_m3`, `poisson`, and `loss_factor`, each with a source or a note of how it was measured, and a price per kilogram. Real panel materials are in (below); the exciters folder still holds only a synthetic test record, and real exciters are still to be sourced.

`catalog/materials.json` holds density and price per kilogram for each print material. Typical densities are 1240 kg/m^3 for PLA and 1270 kg/m^3 for PETG. Confirm them against the filament's own data sheet. The price per kilogram comes from a seller's page. The owner prints with any filament, so the default material is the cheapest one sourced.

`catalog/printers/` holds the owner's printer, a Bambu Lab X2D: its build volume and maximum flow, copied from the maker's spec sheet. Cabinets are split into parts that fit the main nozzle's build volume. The sheet's flow is a maximum measured on a thin-walled test print, so a print time computed from it is a lower bound, shown as "at least".

`catalog/stock/` holds off-the-shelf building materials, one record each, with the same sourcing rules as drivers: a `tier` (`budget` such as PVC pipe, cardboard concrete-form tube, foam board, coroplast, or plant pots; `reference` such as MDF and cabinet plywood), the published sizes it is sold in, its price per piece, per meter, or per sheet with a `read_on` date, its density, and any published stiffness (`E`) and loss factor. A field the seller or maker does not publish stays empty, and the engine says which results depend on it.

### 9.2 Cost

```
cost of one speaker = drivers and exciters + cabinet + panels + extras
cabinet             = printed parts + stock parts
printed parts       = print_mass_kg * price_per_kg
print_mass_kg       = solid_volume_m3 * density_kg_m3 * infill_fraction
print_time_h        = solid_volume_m3 * infill_fraction / printer_flow_m3_per_s / 3600   (a lower bound: the flow is the printer's maximum)
panels              = printed like the cabinet, or bought sheet area * price_per_m2
stock parts         = whole pieces bought: pipe by the length sold, sheets by the sheet, including offcuts
cost of the system  = (2 if stereo_pair else 1) * cost of one speaker + electronics
electronics         = the amplifiers and DSP devices needed to give every radiator its own channel
```

Cost is shown beside performance at all times. Every figure in it traces to a catalog record with a source and a date.

**Cheap against reference.** Any design can be paired with its reference build: the same layout with the closest `reference` drivers and amplifier, and a conventional MDF or plywood box of the same internal volume. The lab shows both scores and both costs side by side, so the owner sees how close the cheap build gets and what the difference costs.

**Where the money matters.** For each part of a design (each driver, the amplifier, the DSP, and the cabinet material), the lab finds the next catalog option up and the next one down. It reports the change in score and the change in cost for each swap, sorted by score gained per dollar. That shows which single part limits the sound and what it would cost to fix it.

## 10. Listen mode

The user hears the simulated speaker and switches between designs.

- The AudioWorklet runs the design's own DSP chain in the `seatlock-rt` kernel and outputs one channel per radiator, up to 16. This is the part that would run on real hardware. The simulation itself does not cap how many drivers or vents a design may have.
- The virtual speaker follows it. Each radiator's channel is convolved with that radiator's simulated impulse response, from amplifier voltage to pressure at the listening position, and the results are summed. The engine computes the impulse responses. Use one `ConvolverNode` per radiator with `normalize = false`, because the default rescales the response. When the design uses a passive R/L/C network (`dsp.passive`), that network is part of the impulse (not the worklet), and per-radiator digital crossover blocks are left out of playback; the filter text says so.
- Sources are an audio file the user opens and built-in test signals: pink noise, sweeps, tones. Do not bundle music.
- A/B: two designs are loaded at once and switched with a short crossfade, at matched level by default.
- A volume control maps to amplifier voltage. The interface shows the simulated level at 1 m and lights a limit indicator when cone travel, opening air speed, or amplifier voltage would exceed section 5.7.
- From Level 3 the virtual speaker reproduces limit behavior audibly, so an overdriven design sounds overdriven.
- From Level 3, when the design has a room, the virtual speaker includes the room estimate, so a radiator that faces away from the listener can be heard doing its job.
- Parameters reach the kernel as messages. The kernel applies them at a block boundary with a crossfade.

Things that go wrong if ignored:

- Browsers do not start audio until the user clicks or taps. Create or resume the audio context inside a click handler.
- The worklet processes fixed blocks of 128 frames. Do nothing in it that can wait.
- Tell the user what they are hearing: the simulated speaker played through their own headphones or speakers, which add their own sound. Differences between designs are reliable. Absolute tone is not.

## 11. Score, targets, optimizer

### 11.1 Metrics

All metrics use the corrected, summed, on-axis response, smoothed to 1/6 octave and normalized to its mean level from 300 Hz to 3 kHz.

```
f6_hz         lowest frequency at which the response is at -6 dB
flatness_db   RMS difference from the target between 1.5 * f6_hz and 16 kHz, on the logarithmic grid
max_spl_db    minimum of max_spl(f) from f6_hz to 16 kHz: the loudest sine the speaker reproduces
              within every limit at every frequency in its range, at 1 m, one speaker
cost          section 9.2
```

**The listening window** (as built, Level 3). A speaker is not heard only from straight ahead, so evenness is judged a little off axis too. A request with `listening_window` computes the direct sound at nine points at the listening distance: straight ahead, 10, 20, and 30 degrees to either side, and 10 degrees up and down, and averages them as energy. `flatness_db` is then the worse of two: the evenness of the level straight ahead (in the room, when the design has one) and the evenness of the listening window, each normalized and banded as above. The results give both. Correction still fits the level straight ahead. Under `exterior` radiation the window is left out (each direction would be a 3D run) and the label says so. The page, scores, the optimizer, and the generator ask for it; the engine's default does not. CHK-WIN-1.

**CTA-2034 preference rating** (as built, Level 3). A request with `cta2034` forms the CTA-2034 early-reflections average under the baffle model — the standard bounce groups (floor 20°/30°/40° down, ceiling 40°/50°/60° up, front wall 0° and ±10°/20°/30°, side walls ±40°…±80°, rear wall 180° and ±90°; 26 directions), a relative sound-power curve (on-axis level minus the directivity index), and the predicted in-room response (12 % listening window, 44 % early reflections, 44 % sound power, as energy). From those curves it computes Olive's preference rating (AES 6190): `12.69 − 2.49·NBD_ON − 2.99·NBD_PIR − 4.31·LFX + 2.32·SM_PIR`. The page asks for it with the listening window and shows the rating and the predicted in-room graph. Scores, the optimizer, and the generator also ask for it: preference is the ranked quality score (section 11.2). Listening-window and early-reflection directions share one batched baffle transfer (outlines built once). Checked: Olive's published Criton 3TD-X terms recover 4.68; from ASR's Criton CTA-2034 file the lab gets LFX ≈ 42 Hz and preference within a few tenths of 4.68 (resampling details in other pipelines leave a small gap).

From Level 3 the results also include sound power, a directivity index, and, when the design has a room, the in-room response of section 5.10. A design with a room is scored on its in-room response, using the target's tilt. A design without one is scored on its direct response.

### 11.2 Targets

A target is a set of thresholds in `targets/`. A design meets the target when it satisfies all of them.

```jsonc
// targets/hifi-default.json
{
  "preference_min": 5.0,
  "f6_hz_max": 50,
  "flatness_db_max": 1.5,
  "max_spl_db_min": 95,
  "budget_max": null
}
```

`preference_min` is the Olive preference floor (AES 6190; higher is better; published kit scores are often about 2 to 7). Default 5.0 — a solid hi-fi bar; change the target file to raise or lower it. `f6_hz_max` and `flatness_db_max` stay for correction and reporting; bass and evenness already feed preference (LFX, NBD, SM). `max_spl_db_min` stays in the shortfall because loudness is not in Olive's formula. The owner confirmed the older bass / evenness / level numbers on 2026-11-11; preference as the ranked score followed on 2026-11-15.

Among designs that meet the target, the cheaper one is better. Among designs that miss it, the one with the smaller shortfall is better:

```
shortfall = max(0, preference_min - preference)
          + max(0, max_spl_db_min - max_spl_db) / 6
```

### 11.3 Optimizer

- It answers one question: what is the cheapest design that meets this target?
- Level 2 variables, for drivers the user has chosen: sealed or vented, box volume, tuning, crossover frequencies and slopes, driver gains. Correction runs inside every evaluation.
- Level 3 adds the choice of drivers from the catalog, cabinet proportions, and internal elements.
- Method: for each combination of discrete choices, search the continuous variables with a derivative-free method on the `lumped` or `waveguide` model. Keep the best few. Verify the finalists with `fdtd` and report what changed.
- Output: a ranked list and a cost-against-quality plot. Every result is a complete design file the user can open.
- It runs in workers, shows first results within 30 s, can be canceled, and is repeatable from its seed.
- As built at Level 2: the discrete choices are sealed or vented, and LR4 or LR2 crossovers for designs with more than one radiator; each combination runs in its own worker. Nelder-Mead searches net air volume (keeping the cabinet's proportions and build method), port tuning (the port's diameter is kept, or 0.35 of the cone's diameter; its length follows from the tuning), crossover frequency, and the levels of radiators after the first, 30 candidates per combination by default. Every candidate is corrected and scored with `lumped` and `baffle` radiation. A port that does not fit the cabinet's depth rules a candidate out. The top finalist is checked with `fdtd` at 10 mm, and the page reports how its bass limit, evenness, and clean level changed.
- As built for a passive board (`dsp.passive`): one combo searches only the R, L, and C values (section 7). The cabinet is not reshaped, so multi-chamber and imported kits work. Candidates are scored as they stand (no digital correction). The page button reads "Tune the passive board"; the full 3D check is skipped because the cabinet did not change.

### 11.4 Generator

- It answers a wider question than the optimizer: which cabinets, of any form the lab can make, come closest to the target for the least money?
- A candidate is a whole design: an outer form and its proportions, an inside layout, the drivers from the catalog, the face each sits on, and where on that face.
- It only uses catalog parts it could really build with: sourced records with the mounting sizes a cut-out needs. Test parts are never proposed (section 9.1).
- Method: candidates are grouped into families (one outer form, one inside layout, one set of drivers). Each family is searched with a seeded random search and local refinement on the everyday models, corrected and scored like any design. Candidates the builder cannot make or print are thrown out. The best are checked with the 3D models, and the page says what changed.
- A candidate whose sound sources do not all face the listener is judged in a room (section 5.10), because its direct sound alone says little.
- Output: a ranked list across all families and a cost-against-quality plot. Every result opens as a design.
- It runs in workers, one per family, shows first results within 30 s, can be canceled, and repeats from its seed.
- As built: one driver, or a woofer and a tweeter. The one-driver families are six printed outer forms (box, rounded box, chamfered box, leaning front, a front that narrows toward the top, and an egg, the `revolved` shape with its driver in the middle of the flat front and its port through the flat back), each sealed and ported; a box with a folded, tapered line; and a box, sealed and ported, with its driver facing up or facing back. Inside a family the search picks the driver, the air volume (1.5 to 40 L), the proportions, where the driver sits on its face, the port's tuning or the line's size and exit height, and how strongly the form is shaped. Three of every five of a family's candidates are random and the other two refine its best two so far, with moves that shrink over its first 40 candidates; nothing depends on how many it is given, so a longer search starts with a shorter one's candidates and never ends worse. A candidate the cabinet report cannot print costs a try, not a scoring. Seven more families are two-ways: a woofer and a tweeter stacked on the front of a sealed box, a ported box, a ported rounded box, a ported leaning front, and a box with a folded line, and a ported box whose woofer faces up or back with the tweeter in front. Each has its own channel, split by a fourth-order Linkwitz-Riley crossover; the search also picks the tweeter, the crossover frequency (from twice the tweeter's resonance, at least 1.5 kHz, up to 4.5 kHz), and the tweeter's level. The drivers are the sourced budget parts whose records give a cut-out and a flange size. Four more families add sound sources, all in a box on the front: two woofers around a tweeter (woofer, tweeter, woofer, the two woofers in one chamber on the same settings), sealed and ported, and three-ways, sealed and ported, with a midrange (the catalog's full-range units) in its own sealed chamber above a partition over the woofer, the tweeter in front of that chamber too. A three-way's bass driver is a woofer; its lower crossover runs from twice the midrange's resonance (at least 250 Hz) to 900 Hz or an octave below the woofer's measured break-up, and the upper one from 2.5 times the lower (at least 1.5 kHz) to 4.5 kHz or an octave below the midrange's break-up; the midrange's level is searched too. A woofer on the back with another in front is not proposed yet. The page runs 12 candidates per family in up to six workers and checks the top cabinet with the 3D model of the air inside, or of the air outside when its driver does not face the listener.

## 12. Build plan

Tracks:

```
A  Design model and app shell     B  Catalog and cost           C  Radiators and fast enclosure models
D  3D solver                      E  Cabinet builder            F  Crossover and DSP chain
G  Correction and auto EQ         H  Listen mode                I  Score and optimizer
```

**The level rule.** Every track advances through the same levels. A level is finished when every track has delivered its items for that level and the level's end-to-end check passes. Do not start any track's next-level items while another track is unfinished at the current level. Inside a level, connect the thinnest version of every track first, then deepen them. Section 2.3 gives the reason.

**Level 0, scaffold.** The layout of section 3.1 exists. `./start.sh` and `./check.sh` work from a fresh clone. The engine builds to WebAssembly, loads in a worker, and answers a call. The worklet plays a test tone produced by the real-time kernel. A design file loads, saves, and round-trips unchanged. Versions are pinned and recorded.

**Level 1, one driver in a sealed box, end to end.**

- A: edit every Level 1 field; save and load; graphs for response, impedance, and cone travel; fidelity labels shown; every family of section 3.3 goes through its registry. CHK-EXT-1, CHK-EXT-2.
- B: the two test drivers of section 13; at least three sourced real drivers if you have web access; cost from drivers and print material.
- C: `cone` radiator, `lumped` sealed box, `point` radiation, limits, maximum level. CHK-DRV-1, CHK-DRV-2, CHK-BOX-1, CHK-BOX-2.
- D: solver core on a closed box; voxels from the `box` outer shape; `Z_ab` from `fdtd` drawn over the `lumped` curve. CHK-FDTD-1, CHK-FDTD-2.
- E: `box` outer shape with one driver cut-out, 3D view, air volume read from the solid, STL export. CHK-GEO-1, CHK-GEO-2.
- F: one-driver chain with high-pass, gain, and EQ biquads; chain response; coefficient export. CHK-DSP-1, CHK-DSP-3.
- G: C1 and C2, with the keep-only-if-it-helps rule. CHK-EQ-1, CHK-EQ-2, CHK-EQ-3.
- H: a music file or test signal through the chain and the virtual speaker; correction switched on and off; a cone-travel meter.
- I: `f6_hz`, `flatness_db`, `max_spl_db`, and cost on screen; a one-variable sweep, for example cost and `f6_hz` against box volume. CHK-COST-1.
- End to end, CHK-E2E-1: the example design `sealed-test` loads, simulates, corrects, exports STL and filters, and reports score and cost. All outputs match stored expected values, and the headless browser shows the design.

**Level 2, a two-way vented speaker from real parts.**

- A: several radiators per design; two designs side by side.
- B: at least twelve sourced drivers across woofers, tweeters, and full-range units, with mounting data, at least nine of them `budget` and at least three `reference`; sourced `budget` amplifier boards and DSP options and one `reference` amplifier; stock materials (PVC pipe, cardboard tube, foam board, MDF, plywood); cost with electronics, stock parts, and stereo pairs; price dates shown.
- C: `lumped` vented box; `waveguide` for ports and straight or tapered ducts; `baffle` radiation with edge diffraction, piston directivity, and separate source positions; published response curves. CHK-BOX-3, CHK-BOX-4, CHK-WG-1, CHK-WG-2, CHK-WG-3, CHK-RAD-1, CHK-RAD-2.
- D: openings with exterior blocks; `H_k`; background jobs with progress and cancel; two-resolution uncertainty. CHK-FDTD-3, CHK-FDTD-4.
- E: outer shapes with rounded and chamfered edges, a sloped baffle, `prism`, `tube` with printed end caps (a hybrid build), and `sheet_box` cut from sheet stock with a cut list; round and slot ports, several chambers, recesses and bolt holes from mounting data, split into printable parts, 3MF export, print time.
- F: Butterworth and Linkwitz–Riley crossovers, fractional delay, automatic polarity, per-radiator limiter; passive R/L/C networks for kit-style crossovers (graphs, scores, listen impulses, the page netlist editor, and R/L/C value search toward the score as built; C-Note MT kit copy vs ASR). CHK-DSP-2.
- G: C3 and C4; EQ per radiator and for the whole speaker.
- H: all radiators playing; A/B between two designs at matched level; live limit indicators.
- I: optimizer over continuous variables for chosen drivers; cost-against-quality plot; cost beside every score; the cheap-against-reference comparison of section 9.2.
- End to end, CHK-E2E-2: the example design `two-way-vented` passes the same sequence as CHK-E2E-1, and its three enclosure models agree within 1.5 dB below 200 Hz.

**Level 3, novel geometry and advanced correction.**

- A: undo and redo; design history.
- B: exciters and panel materials, damping materials, wall fills, and foaming filaments; stale-price warnings; alternative vendors per part.
- C: folded and tapered lines, coupled chambers, drivers mounted on ducts, damping fill, several drivers sharing a chamber, passive radiators; the `dml_panel` radiator with open and sealed backing, layered and direction-dependent panels; the `simple` room; a model of wall vibration, so filled and two-material walls are heard (built for printed cabinets with one or several chambers, including braces that split wall spans and partitions between chambers; imported mesh cabinets' axis-aligned flat outer walls; sheet boxes whose sheet publishes stiffness and density; and tubes' end caps; section 5.11). CHK-PLATE-1, CHK-PLATE-2, CHK-ROOM-1, CHK-WALL-1, CHK-WALL-2.
- D: any internal geometry the builder can make; fill and wall losses; the time budgets of section 5.5 met; `exterior` radiation for any outer shape and any facing; a boundary-element model of the air around (and then inside) the cabinet, joined to the driver network, giving every direction from one solve per frequency (section 2.6); a 3D grid that follows curved surfaces instead of stair-stepping them (tried, section 5.6; no clear gain yet); the pressure field inside the cabinet shown in the 3D view, as a pulse in slow motion and as the standing-wave pattern at a chosen frequency; a bass-only 3D wave model of the room (below about 300 Hz, coarse cells), shown the same way, beside the `simple` room estimate.
- E: editor for ducts with bends and tapers, partitions, braces, and fill regions; printability warnings; `revolved` and `loft` outer shapes; mounts on any surface.
- F: FIR blocks and linear-phase crossovers, with latency shown.
- G: C5, C6, C7, C10.
- H: audible limit behavior and the room estimate in the virtual speaker.
- I: the generator of section 11.4, for any number of sound sources; the CTA-2034 curves and Olive preference rating as the ranked target score (plus clean level); optimizer chooses drivers, build method, and stock materials from the catalog as well as geometry and DSP; the where-the-money-matters view of section 9.2; finalists verified with `fdtd`; designs with a room scored on the in-room response.
- End to end, CHK-E2E-3: the example design `folded-line` runs in all three enclosure models, and the report lists what each model ignored. The example design `rear-panel`, a rear-facing DML panel on an outer shape that is not a box, runs with `exterior` radiation and the `simple` room, and its in-room response differs from its direct response.

**Level 4, closing the loop with real speakers.** Import measurements (FRD and ZMA text files, impulse responses), fit the model to a measured impedance curve, C9, C8, export for the owner's chosen playback hardware, STEP files and stock-sheet parts for imported speakers, and panels that open into a cabinet chamber. Plan this level with the owner when Level 3 is done.

## 13. Checks

Each check becomes an automated test named after its id. The numbers were produced on 2026-11-09 by a separate reference implementation of the equations in section 5, including a small 3D solver. Identity checks (CHK-DRV-2, CHK-BOX-1, CHK-BOX-3, CHK-DSP-2) compare the implementation with closed-form results that are known independently of this code. The value checks anchor your implementation to the same equations. Stored expected values in the end-to-end checks catch regressions only. They never prove correctness.

Test parts:

```
test-woofer-a   fs 50 Hz, Qes 0.45,   Qms 4.0, Vas 10 L, Re 6.0 ohm, Le 0.5 mH, Sd 85 cm^2,  Xmax 4 mm, Pe 30 W
test-woofer-b   fs 40 Hz, Qes 0.4144, Qms 5.0, Vas 20 L, Re 6.0 ohm, Le 0,      Sd 130 cm^2, Xmax 6 mm, Pe 50 W
test-amp-a      1 channel, 50 W into 4 ohm
test-panel-a    0.40 x 0.30 m, 3 mm thick, E 3.5 GPa, 1240 kg/m^3, Poisson's ratio 0.35
test-panel-material-a   the same material as a catalog record: E 3.5 GPa, 1240 kg/m^3, Poisson's ratio 0.35, loss factor 0.03
test-exciter-a  Re 4.0 ohm, Le 0.05 mH, Bl 3.0 T*m, coil 1.5 g, magnet 60 g, suspension 3000 N/m and 1.0 N*s/m, travel 1 mm, Pe 10 W
test-tweeter-a  fs 900 Hz, Qes 0.8, Qms 2.5, Vas 0.05 L, Re 4.8 ohm, Le 0.05 mH, Sd 8 cm^2, Xmax 0.3 mm, Pe 20 W, sealed back
```

Unless a check says otherwise: 2.83 V RMS at the terminals, radiation model `point` with space `half`, distance 1 m, enclosure model `lumped`, no leak, no port loss.

| Id | Setup | Must hold |
| --- | --- | --- |
| CHK-DRV-1 | Derive the mechanical parameters of both test drivers. | A: cms 9.7598e-4 m/N, mms 0.010381 kg, rms 0.8154 N·s/m, bl 6.594 T·m, qts 0.4045. B: cms 8.3449e-4, mms 0.018971, rms 0.9536, bl 8.309, qts 0.3827. Within 0.1 %. |
| CHK-DRV-2 | A with Le set to 0, sealed 5.000 L, at 1 kHz. | SPL equals `10*log10(RHO0*C0*eta0*(V^2/re) / (2*pi*r^2) / P_REF^2)` with `eta0 = 4*pi^2*fs^3*vas / (C0^3*qes)`. That is 87.75 dB, within 0.02 dB. |
| CHK-BOX-1 | A with Le set to 0, sealed 5.000 L. | fc = 86.60 Hz and Qtc = 0.7006. The response equals the second-order high-pass `s^2 / (s^2 + s*wc/Qtc + wc^2)` scaled to the level the CHK-DRV-2 formula gives (use the formula, not the rounded number), within 0.001 dB from 5 Hz to 20 kHz. |
| CHK-BOX-2 | A as published, sealed 5.000 L. | 40 Hz: 74.02 dB, 7.87 ohm, x_peak 1.381 mm. 86.60 Hz: 84.66 dB, 59.33 ohm, 1.003 mm. 1 kHz: 87.12 dB, 6.50 ohm, 0.0100 mm. Within 0.02 dB and 0.5 %. |
| CHK-BOX-3 | B, vented, V_box = vas/sqrt(2) = 14.142 L, one port of radius 17.5 mm with L_eff = 126.86 mm, so f_b = 40.00 Hz. | The response equals a fourth-order Butterworth high-pass at 40 Hz scaled to the level the CHK-DRV-2 formula gives for B (88.21 dB), within 0.001 dB from 8 Hz to 800 Hz. `H_port -> 1` as `f -> 0`. |
| CHK-BOX-4 | Same as CHK-BOX-3. | 20 Hz: 64.11 dB, x_peak 3.462 mm, port air speed 7.84 m/s peak. 40 Hz: 85.20 dB, impedance 6.00 ohm, x_peak under 0.001 mm, port 11.11 m/s. 80 Hz: 88.19 dB, 0.866 mm, 1.96 m/s. Impedance peaks at 22.8 and 70.4 Hz. Within 0.02 dB, 0.5 %, and 0.2 Hz. |
| CHK-WG-1 | Lossless duct 0.5 m long, rigid far end. | Input impedance is zero at 171.6 and 514.8 Hz and unbounded at 343.2 Hz. |
| CHK-WG-2 | 2 L chamber, neck radius 10 mm and length 30 mm, flush with the chamber wall inside, free end outside. | Resonance within 0.5 % of the `lumped` value, 102.5 Hz. |
| CHK-WG-3 | The CHK-BOX-3 box with its port as a duct of physical length 101.27 mm, flush with the chamber wall inside, free end outside. | SPL within 0.1 dB of `lumped` below 200 Hz. First pipe resonance of the port, a peak in `H_port`, at 1355 Hz within 2 %. |
| CHK-FDTD-1 | Rigid box 0.30 x 0.20 x 0.15 m, dx = 10 mm, source patch of 4 x 4 cells near a corner of a 0.20 x 0.15 wall. | Peaks of `Z_ab` at 572.0, 858.0, 1031.2, 1144.0, 1279.0, and 1430.0 Hz, each within 0.5 %. |
| CHK-FDTD-2 | Same box. | At 10 Hz, `Z_ab` is within 1 % of `1 / (omega*C_ab)` in magnitude and within 1° of -90° in phase. |
| CHK-FDTD-3 | Cavity 0.20 m cube. Neck 40 x 40 mm, 100 mm long, centered on one wall, opening into an exterior block with absorbing outer faces. Source patch 60 x 60 mm centered on the opposite wall. dx = 10 mm. | The run is stable. The peak of `Z_ab` is within 2 % of 65.7 Hz. `H_port` is 1.024 at 10 Hz and 1.103 at 20 Hz, real part within 1 %, imaginary part under 0.05 in size. |
| CHK-WG-4 | `waveguide` with losses, a round duct 0.5 m long of radius 20 mm, rigid far end. | At the quarter wave (171.6 Hz) the input impedance is the pure resistance `Z0 * tanh(alpha*L)` with `alpha = 2.99e-5 * sqrt(f) / a`, within 0.01 %. At the half wave its size is `Z0 / tanh(alpha*L)` within 0.1 %. |
| CHK-FDTD-4 | `fdtd` at 5 mm against `waveguide` for the CHK-BOX-3 cabinet as the builder makes it. | Tuning frequency within 3 %. SPL within 1.5 dB below 300 Hz. |
| CHK-FDTD-5 | The CHK-FDTD-1 box with a uniform loss `s` of 10 and 30 per second. | The first resonance's peak in `abs(Z_ab)^2` is wider at half height by `s/pi` Hz than without the loss, within 3 %. With the early stop the peak is at the same frequency within 0.11 Hz and as wide within 3 %. |
| CHK-DSP-1 | 48 kHz. Lowpass at 1 kHz with Q 0.7071. Peaking at 1 kHz, +6 dB, Q 2. | -3.01 dB and +6.00 dB at 1 kHz, within 0.01 dB. |
| CHK-DSP-2 | 48 kHz, crossovers at 2 kHz. | LR4: each side is -6.02 dB at 2 kHz, and low plus high is 0 dB within 0.001 dB at every frequency. LR2: the sum is 0 dB within 0.001 dB only with one side inverted, and has a deep null at 2 kHz in phase. |
| CHK-DSP-3 | Any block, fed an impulse through its process function. | The FFT of the output equals the block's reported frequency response within 0.01 dB and 0.1°. Block sizes 1, 128, and 4096 give identical output. |
| CHK-EQ-1 | C1 on a flat response with one added peak: +5 dB, Q 2, at 1 kHz. | One cut, within 0.5 dB in gain, 5 % in frequency, and 20 % in Q. Remaining flatness under 0.2 dB. |
| CHK-EQ-2 | A in the sealed 5 L box, default target, after C1 and C2. | No boost inside the protected band. A sine at the design level, at any frequency from 10 Hz to 20 kHz, stays within every limit of section 5.7. |
| CHK-EQ-3 | C1 on an already flat response. | No filters, and a message saying why. |
| CHK-GEO-1 | Every example design. | Air volume by formula and from the print solid agree within 0.5 %. Voxels at 5 mm agree within 2 %. |
| CHK-GEO-2 | Every exported print file. | Each edge is shared by exactly two triangles, winding faces outward, volume is positive, coordinates are in millimeters, and re-importing gives the same volume within 0.1 %. |
| CHK-COST-1 | Every example design. | Cost equals a hand calculation written out in the test. |
| CHK-EXT-1 | A test-only crate registers a new outer shape, a new radiator, and a new DSP block, then runs `simulate` on a design that uses them. | It works, and no file outside that crate changes. |
| CHK-EXT-2 | A design file containing a part of a kind this build does not know. | It opens, saves unchanged, simulates the rest, and reports a warning naming the unknown part. |
| CHK-RAD-1 | `baffle` radiation, a rigid piston of radius 50 mm on a very large flat surface, at `k*a = 3` (3277 Hz). | 30° off axis the level is 2.57 dB below the on-axis level, and 60° off axis it is 8.81 dB below. Within 0.1 dB. |
| CHK-RAD-2 | `baffle` radiation, a point source at the center of a circular baffle of radius 0.10 m, listener on axis at 100 m. | Relative to `half` space: -6.02 dB at 10 Hz, +3.52 dB at 1716 Hz (where `k*R = pi`), and -6.02 dB at 3432 Hz (where `k*R = 2*pi`). Within 0.05 dB. |
| CHK-RAD-3 | `exterior` solver, a rigid sphere of radius 0.1 m in a plane wave, `k*a` from 0.18 to 6, at 10 mm and 5 mm cells. | Against the closed-form series `p = -j/(ka)^2 * sum (2m+1) j^m P_m(cos theta) / h_m'(ka)`: within 0.75 dB at the point facing the wave and at 90 degrees. At the shadowed point within 3.6 dB at 10 mm and 1.8 dB at 5 mm (stair-steps on a curved surface). |
| CHK-BEM-1 | `seatlock-bem` exterior CBIE, pulsating sphere radius 0.1 m, 320-face icosphere, `ka` 0.5 / 1 / 2. | Surface `|p|` within 0.5 dB of `ρ c v_n · j ka / (1 + j ka)`; phase within 10°. |
| CHK-BEM-2 | `seatlock-bem` rigid sphere radius 0.1 m in a unit plane wave, 320-face icosphere, `ka` 0.25 / 0.5, lit / side / shadow. | Surface `|p|` within 0.75 dB of the CHK-RAD-3 closed-form series (same budget as CHK-RAD-3 lit/side). Higher `ka` not yet claimed. |
| CHK-RAD-4 | `exterior` solver, a box 0.2 x 0.16 x 0.24 m, 10 mm cells, one skin cell on its front, one point beside it and one behind it. | The reciprocity run and a plain radiation run the other way round agree within 7 % and 10 degrees at 50, 200, 500, 1000, 2000, and 3000 Hz. |
| CHK-WIN-1 | `baffle` radiation with `listening_window`, test-woofer-a's piston at the middle of a 40 x 40 m surface, listener at 100 m, 41 frequencies from 3.0 to 3.6 kHz. | The window's level against the level straight ahead, averaged over the frequencies, equals the energy mean of the piston's directivity over the nine directions, within 0.05 dB. Under `point` radiation the window equals the level straight ahead within 0.02 dB. |
| CHK-PLATE-1 | `test-panel-a`, all four edges simply supported, no exciter, no loss. | The first five resonances are 42.36, 88.10, 123.68, 164.35, and 169.43 Hz. Within 1 %. |
| CHK-PLATE-2 | An orthotropic plate 0.40 x 0.30 m, 4 mm thick, `E_u` 3.74 GPa, `E_v = 0.015*E_u`, `G = 0.037*E_u`, `nu_uv` 0.488, 179 kg/m^3, simply supported, shear through the thickness left out. | Every resonance below 95 % of the resolved limit matches `omega^2 = (D11*p^4 + 2*(D12 + 2*D66)*p^2*q^2 + D22*q^4)/m`, `p = i*pi/a`, `q = j*pi/b`, within 1 %, with at least 25 of them. |
| CHK-ROOM-1 | `simple` room of 50 m^3 with rt60 0.4 s, no wall reflection, a source radiating 1 W equally in all directions. | A = 20.12 m^2. Diffuse level 113.12 dB. Direct level at 2.5 m 101.19 dB. Within 0.05 dB. |
| CHK-WALL-1 | A wall 0.152 x 0.242 m of 8 mm PLA (E 2.68 GPa, nu 0.3, 1240 kg/m^3, loss factor 0.02), supported edges, under uniform pressure. | Every resonance the pressure drives, and its shape's integral, against Navier's series (`f_ij` from `(i*pi/a)^2 + (j*pi/b)^2`, `s_ij^2 = 64*a*b / (m*pi^4*i^2*j^2)`, odd `i`, `j`): frequency within 0.1 % below half the resolved limit and 1 % above, integral within 0.5 % and 5 %. Outward volume velocity per pascal at 5 Hz, half the first resonance, the first three resonances, and between the first two, within 1 % of the series. Static compliance within 1 % of the series to 400 terms. |
| CHK-WALL-2 | Two lossless 1.6 mm PLA skins on a 4.8 mm core, E 4.1 MPa, loss factor 0.5, wavenumbers 5 to 320 per m. | The stack's complex stiffness equals the hand-assembled `D_local + D_core*(1 + j*beta) + D_sandwich*g/(1 + g)` within 1e-9, and its loss factor the Ross-Kerwin-Ungar formula within the core's own bending loss, `beta*D_core/D_local`. |
| CHK-E2E-1, 2, 3 | Section 12. | As stated there. |

## 14. Interface

- Plain language first. Labels use everyday words, with the technical term in the explanation, for example "Cone travel (excursion)". Section 16 lists the pairs.
- Every control and every graph has a one-sentence explanation on hover or tap.
- Every number shows its unit. Every graph shows its fidelity label. Every warning appears beside its cause and says what to do about it.
- Editing feels instant. `lumped` and `waveguide` results update within 50 ms of a change. Slower work shows progress and can be canceled. Results that no longer match the design are grayed out until they are recomputed.
- Interface work never makes the audio glitch.
- Frequency axes are logarithmic, 20 Hz to 20 kHz by default.
- The everyday enclosure model is `waveguide` ("Detailed"): as fast as `lumped`, the same for a box without ducts, and it knows lines, tapers, and pipe resonances. The page, correction, scores, the optimizer, and listen mode all use it.
- "Generate cabinets" runs section 11.4's generator: progress per family, a ranked table with an Open button per cabinet, the 3D check of the top one, and the cost-against-quality plot.
- Evenness is shown with its two parts: straight ahead, and a little off to the sides and up and down.
- "Shape the inside" runs section 5.5's shaping in a worker, at 20 mm or 10 mm blocks, with progress and a stop button. It shows the unevenness before and after, the plastic added, and the level from the inside's shape for both, and "Use this inside" puts the plastic into the design.
- The cabinet's inside is edited on the page: air spaces (chambers) by their corners, and air paths by their two ends, shape, sizes at each end (a taper), and their corners in mm, one per line.
- A passive crossover board is edited on the page: turn it on or off, name the amplifier nodes, and add or change resistors (ohm), inductors (mH), capacitors (µF), and which driver each load is. When a board is on, the cost panel's optimizer tunes those part values toward the target instead of reshaping the cabinet.
- Extra drivers are added on the page (each with its own opening). Vents are air paths that open outside; add as many as the experiment needs. Play follows up to 16 drivers.
- The app asks for `baffle` radiation by default, because a real cabinet loses bass to the baffle step. `point` in `half` space is for checks, which name their own model. "Run the outside in 3D" runs `exterior` radiation at 10 mm and then 5 mm in a worker and draws it over the everyday model, with the in-room curve when the design has a room. A design with a source that faces away from the listener gets a warning that sends the user to that button.
- The screen holds design controls, the 3D view and graphs, score and cost, and a listen bar that is always within reach. Narrow screens stack them.
- Materials are chosen from the catalog by name: the print material (foaming filaments marked), the wall build (solid, two skins filled with sand or gypsum cement, or two skins with a second filament; each heard through the wall model of section 5.11), a panel's material or its layers (with Sandwich and Plywood presets), and an imported damping part's material.
- "Import a speaker" reads a 3MF or STL assembly: a quarter-turn control for its front, a table of its parts (solid in a print material, or damping with its flow resistivity), and buttons that place each sound source or a port by clicking the 3D view, with tilt and turn.
- A design is one file the owner can save, reopen, duplicate, and send to someone.
- A part of a kind this build does not know is shown as unsupported. It is never dropped or hidden.

## 15. Not simulated

Say so in the interface wherever it matters.

- Cabinet wall vibration of curved skin (a tube's pipe included) and of slanted or free-form imported faces. Printed cabinets, divided or not (outer walls and partitions between chambers), imported mesh cabinets (axis-aligned flat outer walls, thickness measured from the solid), sheet boxes whose stock publishes stiffness and density (foam board, MDF, plywood), tubes' end caps, and braces that split wall spans have it (section 5.11), with the limits listed there. Cheap stock materials such as cardboard flex more than printed walls; until their walls are modeled, the builder enforces minimum wall thickness, offers bracing, and warns more strongly for materials with low stiffness.
- The listening room, beyond the simple estimate of section 5.10 that arrives at Level 3. Until then all results are for free space or a baffle.
- For DML panels: the air's load on the panel, an open back, and anything above the frequency the label gives (section 5.9). A panel's result also depends on material numbers that printing makes uncertain.
- Tweeter horns and waveguides, which need a different solver.
- Damping fill in the quick and detailed models (the full 3D model has it for imported damping parts, with one flow-resistivity figure at every frequency, section 5.5), and each resonance's own wall loss (the 3D model applies one average, section 5.5).
- Voice-coil heating over time, and distortion.
- Mutual inductance and transformer models in a passive board (series R, L, and C and driver loads are in; section 7).

## 16. Glossary

| Interface wording | Technical term | Meaning |
| --- | --- | --- |
| Passive crossover board | `dsp.passive` R/L/C network | Parts between one amplifier and the drivers that split the signal, the way a kit crossover PCB works. |
| Driver | Loudspeaker drive unit | A single woofer, tweeter, or full-range unit. |
| Sound source | Radiator | Anything driven by one amplifier channel that makes sound: a cone driver with its enclosure, or a DML panel. |
| Panel speaker | DML panel, `dml_panel` | A stiff, light panel made to vibrate by an exciter. It radiates through many bending resonances at once. |
| Exciter | Inertial exciter | The small motor fixed to a panel that makes it vibrate. |
| Sound power | `W` | All the sound a speaker sends out, in every direction. |
| In-room response | Section 5.10 | The sound that reaches the listener directly and by way of the room. |
| Listening window | Section 11.1 | The sound averaged over straight ahead and a little to the sides, above, and below: what a listener who is not exactly on axis hears. |
| Directivity index | `DI` | How much louder the speaker is straight ahead than on average over all directions. |
| Cone travel | Excursion, `x_peak`, limit `xmax_m` | How far the cone moves from rest. |
| Port air speed | Port velocity | Peak speed of air in a port. Too fast and the port makes noise. |
| Bass limit | `f6_hz` | The lowest frequency the speaker reproduces within 6 dB of its normal level. |
| Evenness | `flatness_db` | How closely the response follows the target: the worse of straight ahead and the listening window. |
| Maximum clean level | `max_spl_db` | The loudest the speaker plays without exceeding a limit. |
| Tuning | `f_b` | The frequency at which a port takes over from the cone. |
| Crossover | Digital isolated crossover | The split of the signal between radiators, done digitally, one channel per radiator. |
| Correction | Auto EQ and the features of section 8 | Digital processing that improves the response within the hardware's limits. |
| Budget part, reference part | `tier` | A budget part is the cheap option the lab is built around. A reference part is a quality component kept for comparison. |
| Stock material | `catalog/stock/` | An off-the-shelf building material such as PVC pipe, a cardboard tube, foam board, or plywood. |
| Hybrid build | Build method | A cabinet that combines stock parts with printed parts. |
| Quick, Detailed, Full 3D | `lumped`, `waveguide`, `fdtd` | The three enclosure models, from fastest to most complete. |
| Sandwich panel | `layers` in a panel | Two thin stiff skins on a light core: far stiffer for its weight than one sheet (section 5.9). |
| Wall build | `build.wall` | How a printed wall is made: solid, two skins filled with sand or gypsum cement, or two skins with a second filament between them. |
| Wall vibration, wall resonance | Section 5.11 | Cabinet walls bending under the pressure inside and sending out sound of their own; strongest at each wall's resonances. |
| Foaming filament | `"foaming": true` in `materials.json` | A filament that foams as it prints, so the part is lighter and less stiff than solid plastic. |
| Independent measurement | `independent` in a driver record | What a respected tester measured on real samples, used in place of the maker's figure and shown beside it (section 9.1). |
| Thiele–Small parameters | `fs_hz`, `qes`, `qms`, `qts`, `vas_m3`, `re_ohm`, `le_h`, `sd_m2`, `bl`, `mms`, `cms`, `rms` | The published numbers that describe a driver: resonance frequency, electrical, mechanical, and total Q, equivalent air volume, coil resistance and inductance, cone area, force factor, moving mass, suspension compliance, mechanical resistance. |
