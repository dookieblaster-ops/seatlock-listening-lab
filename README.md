# Seatlock Listening Lab

Seatlock Listening Lab is a speaker design simulator that runs in the browser: a DIY hi-fi playground. The user picks real, purchasable drivers, shapes a cabinet, both its outer form and its internal geometry, from stock materials, 3D-printed parts, or a mix, sets up a digital crossover and digital correction, and the lab shows and plays what that speaker would do and what it would cost to build. It is a place to try unconventional ideas, such as novel cabinet shapes and rear-facing DML panels, before spending money on them.

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

1. Read this file and `STATUS.md`. When another agent may be active, also read `COMS.md` (section 1.4).
2. Run `./check.sh`. Level 0 creates it; before that there is nothing to run. If it fails before you have changed anything, repairing it comes first.
3. Choose work using the level rule in section 12.
4. Work in small commits. Every commit leaves `./check.sh` passing.
5. Before you stop, update `STATUS.md`, commit, and push.

You start each session with no memory of the last one. Whatever is not in the repository is lost, so `STATUS.md` has to be true at the end of every session.

### 1.3 Rules that protect the simulator

A simulator is worth using only if its numbers can be trusted. These rules exist for that reason.

- **Do not invent data.** Driver parameters, prices, and material properties come from a source you have open in the same session (section 9). Your own memory of a part is not a source.
- **Do not weaken a check to make it pass.** A failing check from section 13 means the code is wrong until shown otherwise. If you conclude the check itself is wrong, prove it with an independent derivation, record the proof in `STATUS.md`, and only then change the check.
- **Label every result with the model that produced it**, and list what that model could not represent (section 5.8).
- **Keep physics out of the interface and the interface out of the physics** (section 3).
- **Keep this README true.** When the owner changes what the product is, or you find the specification wrong, change the README in the same commit as the code and note it in `STATUS.md`.

### 1.4 Multi-agent communications (Claude and Cursor)

When **Claude Code** and a **Cursor** coding agent work in this repository at the same time, they coordinate through `COMS.md`, not through chat the owner may not see.

| Role | Agent | Job |
| --- | --- | --- |
| Lead architect and engineer | Claude | Owns architecture, cross-track order, and changes to this specification. Assigns work and resolves forks that touch more than one track. |
| Helper | Cursor agent | Implements assigned work, runs `./check.sh`, keeps commits small, and reports only verified results. Escalates gaps in the spec to Claude via `COMS.md`. |

**Every session for both agents:** read `README.md`, then `STATUS.md`, then `COMS.md`. Before stopping, post a handoff in `COMS.md` if the other agent should continue the thread.

Message format, thread rules, and the live board are in `COMS.md`. Product decisions for the owner still belong in `STATUS.md` (section 1.1), not in `COMS.md`.

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

"Digital isolated crossover" means every radiator has its own DSP channel and its own amplifier channel. Radiators are electrically isolated from each other. There are no passive crossover parts between an amplifier and what it drives.

### 2.2 What the user does

1. Picks drivers from a catalog of real parts with prices.
2. Shapes the cabinet: an outer shape that can be a plain box or a novel form, walls, chambers, ports, internal ducts and folds, bracing. Sees it in 3D. Gets print files for the printed parts and a cut list for the stock parts.
3. Places radiators on any surface, facing any way, including unconventional ones such as rear-facing DML panels.
4. Sets the crossover, or has the lab propose one.
5. Runs correction and auto EQ.
6. Reads the results: frequency response, impedance, cone travel, port air speed, and maximum clean level, before and after correction, per radiator and summed.
7. Listens to the simulated speaker and switches between designs.
8. Sees what the design costs and how it scores, and asks the optimizer for the cheapest design that meets a target.

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

- Build both WebAssembly modules with `cargo build --target wasm32-unknown-unknown --release` and no other build tool. Both expose a plain C ABI (`extern "C"`): allocate, free, and one call that takes a method name and a JSON request and returns a JSON reply, with raw buffers for large arrays. Do not use `wasm-bindgen` or `wasm-pack`. Reason: the owner's laptop then needs only Rust and Node, with no extra build tool to install and keep at a matching version, and the workers and the worklet load their modules the same way.
- Engine dependencies must be pure Rust. The `wasm32-unknown-unknown` target cannot link C or C++ libraries without extra toolchains.
- The page fetches the real-time module's `.wasm` bytes and posts them to the worklet, which instantiates them there, because the worklet scope has no `fetch`. Only numbers and sample buffers cross that boundary, never text, because the worklet scope may lack the text-decoding APIs.
- Use the current stable release of each tool when you scaffold. Pin versions with `rust-toolchain.toml`, `Cargo.lock`, and `package-lock.json`, and record them in `STATUS.md`. List the `wasm32-unknown-unknown` target in `rust-toolchain.toml` so that rustup installs it by itself.
- Define the design types once, in Rust. Generate the TypeScript types from them. Do not maintain two hand-written copies.

### 3.1 Repository layout (target)

```
README.md          this specification
STATUS.md          current state, decisions, next steps, questions for the owner
COMS.md            shared message board for Claude and Cursor (section 1.4)
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
catalog/           drivers/, electronics/, materials.json
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
              frequency grid, which outputs }
  results:  per radiator and summed: complex pressure response, impedance, cone travel,
            opening air speed, limits, maximum level; score; cost; warnings;
            and for every curve the fidelity label of section 5.8
```

One function turns a design into results. The interface, the optimizer, the checks, and listen mode all use it. The other entry points (correct, optimize, export, impulse response) take a design and return a design, files, or samples.

### 3.3 Extension points

The lab is a playground, so new ideas will arrive after the code exists. Each family below has a contract and a registry. **Adding a member means adding its own files and one registration. It must not require editing the members that already exist.** CHK-EXT-1 enforces this.

| Family | A member must provide | First members (level) |
| --- | --- | --- |
| Outer shape | The print solid or stock parts; an inside test, or a mesh, for the voxel grid; named mounting surfaces that return a position and an outward direction for any surface coordinate; outline data for diffraction. | `box` (1), `prism`, `tube`, and `sheet_box` (2), `revolved` and `loft` (3), imported mesh (4) |
| Internal element | Its air or solid region; its role in the air network (node, edge, or shunt); its primitives for the print solid and the voxel grid. | chamber, duct, solid (1 to 2), fill, passive radiator (3) |
| Radiator | Its electrical impedance; its surface sources per volt (section 5.6); its limits; what it needs from its mount; the catalog parts it uses. | `cone` (1), `dml_panel` (3) |
| Enclosure load model | `Z_ab` and `H_k` (section 5.2). | `lumped` (1), `fdtd` (1), `waveguide` (2) |
| Radiation model | Pressure at any point and radiated sound power, from surface sources and the outer shape (section 5.6). | `point` (1), `baffle` (2), `exterior` (3) |
| Room model | The path from the speaker's radiation to the listening position (section 5.10). | `free` (1), `simple` (3) |
| DSP block | Its frequency response, its sample process function, its parameters. | biquad, crossover, delay, gain, limiter (1 to 2), FIR, dynamic bass (3) |
| Correction feature | Blocks with reasons, from responses, limits, and target, under the keep-only-if-it-helps rule. | C1 to C10 (section 8) |
| Catalog kind | Record shape, sourcing rule, tier, price. | driver, amplifier, DSP device, print material, stock material (1 to 2), exciter, panel material (3) |
| Exporter and importer | Files out and files in. | STL, filter text (1), 3MF (2), FIR WAV (3), measurements and hardware formats (4) |
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

This inverts the CHK-DRV-2 formula and treats the printed figure as half-space, which is close to the large baffle makers usually measure on. It is the least reliable path, because makers often average sensitivity over a band, so the driver is labeled "derived from sensitivity". For a sealed-back driver it still fixes everything the model uses: the level, the shape from `fs` and the Q values, and cone travel through `sd`. When a sheet prints both `vas_m3` and a sensitivity, and the level computed from `vas_m3` differs from the printed sensitivity by more than 1.5 dB, keep `vas_m3` and attach a warning. Record which path was used. If `xmax_m` is not printed, `max_spl` leaves cone travel out for that driver, and a warning says so beside the maximum clean level.

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
- **Openings radiate into an exterior air block** whose outer faces absorb. Start with a block that extends three opening widths in every direction, and confirm that doubling it changes the resonance frequencies, and `Z_ab` and `H_k` more than 5 % away from any resonance, by under 1 %. Do not judge this by peak heights. Through Level 2 the air is lossless, so a port resonance is damped only by radiation: `Q = 2*pi*L_eff / (k*S)`, about 450 for the CHK-FDTD-3 neck, a bandwidth of 0.15 Hz. Its height then depends on the absorbing faces and on the run length, not on the cabinet. Label resonance peak heights as not modeled until losses arrive at Level 3.
- **Stop** when the aperture pressure has fallen below 1e-4 of its peak, or at a time cap.
- **Valid range**: up to `C0 / (10*dx)`, which is 3.4 kHz at 10 mm and 6.9 kHz at 5 mm. A feature thinner than two cells is unresolved, so warn about it.
- **Preserve cross-section areas** when turning round or slanted shapes into cells. Slanted and curved walls become stair-steps, so run at two cell sizes and report the difference as the uncertainty.
- **Self-check every run**: below `C0 / (50 * longest internal dimension)`, `Z_ab` must match `lumped` within 3 %.
- **Time budgets** on the reference machine with one worker: a 20-liter cabinet in under 20 s at 10 mm for a preview and under 5 min at 5 mm for a final result. These are targets. Record measured times in `STATUS.md`.

This model knows the full shape of the internal air, within its grid. Through Level 2 the walls are rigid and the air is lossless. Fill and wall losses arrive at Level 3. It does not model wall vibration.

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

`B(f)` is the baffle gain, from 1 when sound radiates all around to 2 when it radiates forward only. The request's `space` picks it. `full` is `B = 1`. `half` is `B = 2`, an infinite baffle, and is what the checks use. `step` is a shelf centered on `f3 = 115 / baffle_width_m`: `B = (1 + j*f/(f3/sqrt(2))) / (1 + j*f/(f3*sqrt(2)))`. All sources of one radiator sit at one point, and the direction a source faces is ignored. A design with a source that does not face the listener gets a warning under this model.

**`baffle`, Level 2.** Each source sits at its real position. A source on a flat surface that faces the listener gets piston directivity, `D = 2*J1(k*a*sin(theta)) / (k*a*sin(theta))` with `theta` measured from its outward direction, and edge diffraction computed from that surface's outline. This model is not valid for a source that faces away from the listener or sits on a curved surface. For those it falls back to `point` and says so in the fidelity label.

Edge diffraction, the simple secondary-source model: divide the outline of the surface the source sits on into `N` equal edge elements, at least 4 per wavelength at the highest frequency and never fewer than 64. Element `i` is at distance `d_i` from the source and `e_i` from the listening point; the source is at distance `r_0` from the listening point. Then

```
p = p_half_space * ( 1 - (1/(2N)) * sum over i of (r_0 / e_i) * e^(-j*k*(d_i + e_i - r_0)) )
```

where `p_half_space` is the source's pressure with `B = 2`. At low frequency the sum tends to `-1/2`, so the level falls 6.02 dB to that of full space. At high frequency the terms cancel and the level approaches half space. The model ignores the depth of the cabinet and the edge's shape, which the fidelity label lists.

**`exterior`, Level 3.** The 3D solver of section 5.5 run on the air outside the cabinet. The outer shape is a rigid body inside a block of air with absorbing faces, and the surface sources are velocity boundaries on its skin. This is the model for novel outer shapes and for radiators that face sideways, upward, or backward. One reciprocity run, with a point source at the listening position and the pressure recorded all over the cabinet's skin, gives the transfer from every possible surface source to that position at once. Sound power needs such runs for a set of directions around the cabinet. This model is an outline: nothing has validated it yet.

Sound power is the integral of `|p|^2 / (RHO0*C0)` over a sphere in the far field.

The piston model has no cone break-up. Above roughly `k*a = 1`, a driver's real response comes from its published or measured curve. Level 2 adds a response file per driver, blended in above a chosen frequency. Until a curve is present, label that region accordingly (section 5.8).

### 5.7 Limits and maximum level

Everything scales linearly with drive voltage, so compute at 1 V and scale.

```
cone travel         x_peak(f)                      <= xmax_m
opening air speed   sqrt(2) * |U_out_k(f)| / S_k   <= 0.05 * C0   (17 m/s, default)
voice-coil heating  |I(f)|^2 * re_ohm              <= pe_w
amplifier           V(f)                           <= sqrt(amp_power_w * amp_rated_load_ohm)
```

`max_spl(f)` is the SPL at 1 m when the first limit is reached with a sine at frequency `f`. `headroom(f) = max_spl(f) - design_level`, where the design level is the target's `max_spl_db_min` (section 11). The limits above belong to the `cone` radiator and its enclosure. Every radiator kind states its own (section 3.3).

### 5.8 Fidelity labels

Every curve and number carries the models that produced it: the enclosure model (`lumped`, `waveguide`, `fdtd`, or `measured`), the radiation model (`point`, `baffle`, or `exterior`), and the room model. It also carries a list of design features those models ignored, for example "two bends treated as straight", "no cone break-up above 2.1 kHz", "chamber standing waves not modeled", or "rear-facing source treated as a point". The interface shows the label on every graph. When two models are available, draw both. Where they disagree is something the user needs to see.

### 5.9 DML panels, radiator kind `dml_panel`, Level 3

A distributed-mode loudspeaker is a stiff, light panel driven by an exciter. It radiates through many bending resonances at once, which spreads sound widely. A rear-facing panel reaches the listener mostly by way of the wall behind the speaker, so it can only be judged with the `exterior` radiation model and the room estimate of section 5.10.

This section is an outline. Only its closed-form check (CHK-PLATE-1) has been computed. Validate each part as you build it and record what you find in `STATUS.md`.

The panel is a thin plate in bending, with deflection `w`:

```
D = E*h^3 / (12*(1 - nu^2))            bending stiffness, N*m
m = rho*h                              mass per area, kg/m^2
D * del^4(w) + m * d2w/dt2 = pressure difference across the panel + exciter force per area
simply supported panel a by b, mode (i, j):   f_ij = (pi/2) * sqrt(D/m) * ((i/a)^2 + (j/b)^2)
```

Loss enters as a complex stiffness `E*(1 + j*eta)`, with `eta` the material's loss factor. Each edge is free, simply supported, or clamped. Free edges are the usual DML case and have no closed form, so solve the plate numerically: find its modes by finite differences or finite elements, then sum them.

The exciter is an inertial motor. Its coil is fixed to the panel and its magnet hangs on a suspension. With `v_p` the panel velocity at the exciter and `v_m` the magnet velocity:

```
V       = Ze*I + bl*(v_p - v_m)
F_panel = bl*I + Zs*(v_m - v_p)                 Zs = k_s/(j*omega) + r_s, the exciter's suspension
magnet:   j*omega*m_mag*v_m = -bl*I - Zs*(v_m - v_p)
```

The coil's mass is added to the panel at the exciter position.

- **Backing.** `open`: both faces radiate. `sealed`: only the front face radiates, and the trapped air is a spring that puts a uniform pressure `-(RHO0*C0^2 / V_cavity) * integral of w over the panel` on the panel. A panel that opens into a cabinet chamber comes at Level 4.
- **Radiation.** Each panel element is a surface source with velocity `j*omega*w`. An `open` panel also has a back-face source at each element with the opposite sign.
- **Limits.** Panel deflection at the exciter against the exciter's travel, voice-coil heating, and amplifier voltage.
- **Data.** The exciter comes from the catalog (section 9). The panel's `E`, `rho`, `nu`, and loss factor come from a source or a measurement. Printed panels vary with infill and print direction, so the interface says the result depends on those numbers.

```jsonc
{ "kind": "dml_panel", "exciter_id": "...", "mount": "m2", "backing": "open",
  "panel": { "width_m": 0.30, "height_m": 0.20, "thickness_m": 0.003, "material": "...", "edges": "free" },
  "exciter_at_m": [0.12, 0.09] }
```

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

This is an estimate for comparing designs. It is not a room simulation.

## 6. Cabinet builder

### 6.1 What it is

A parametric model of the cabinet. The user edits numbers and drags handles, and the solid is regenerated. There is no free-form sculpting.

### 6.2 Geometry data

**Outer shape.** The cabinet's outside is a member of the outer-shape family (section 3.3). `box` is the Level 1 member. Level 2 adds rounded and chamfered edges, a sloped baffle, and `prism`, which is any closed outline extruded along one axis. Level 3 adds `revolved` shapes such as cylinders, spheres, and eggs, and `loft`, which places cross-sections along a spine. Level 4 adds an imported mesh. The wall follows the outer shape at `wall_m` thickness unless the chambers say otherwise.

**Build method.** Each part of the cabinet is either printed or a stock part from `catalog/stock/`, such as a length of PVC pipe or a cut sheet. A hybrid build combines them: for example, a stock pipe for the body with printed end caps, baffle, and port. The outer-shape member `tube` (Level 2) is a stock pipe or tube cut to length, whose diameter and wall come from its stock record. The member `sheet_box` (Level 2) is a box cut from sheet stock such as MDF, plywood, or foam board, with a cut list. Printed parts and stock parts both feed the air network, the voxel grid, the cost, and the export: print files for printed parts, and a cut list for stock parts.

Printing is optional. A cabinet can be all stock, all printed, or a mix, and the lab prints a part only when that improves the sound or lowers the cost. When the same cabinet can be built more than one way, the lab shows the ways side by side, each with its cost and its results. Printing earns its place through geometry that stock parts cannot make, such as shaped ports, folds, and curved baffles. Wall stiffness also differs between materials, but wall vibration is not simulated (section 15), so the comparison says so.

**Mounts.** A mount names one of the outer shape's surfaces and a position on it. The outer shape returns the mount's position and outward direction, so a radiator can sit on any surface and face any way: front, back, side, top, or along a curve.

**Inside.** Section 4.1 shows the Level 1 enclosure. From Level 2 the same fields describe ports, extra chambers, and internal ducts:

```jsonc
"chambers": [ { "id": "main", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242], "fill": null } ],
"ducts": [ { "id": "port", "from": "main", "to": "outside",
             "path_m": [[0.080, 0.060, 0.050], [0.080, 0.0, 0.050]],   // center line; a corner in the path is a bend
             "section": { "shape": "round", "start_m": [0.035], "end_m": [0.035] } } ],   // round: [diameter]; rect: [width, height]; start != end is a taper
"solids": [ { "id": "brace-1", "kind": "brace", "min_m": [0.008, 0.080, 0.120], "max_m": [0.152, 0.100, 0.130] } ]   // a bar across the chamber
```

The air region is chambers plus ducts minus solids. Chambers are the nodes and ducts are the edges of the air network that `lumped` and `waveguide` solve. From Level 3 a driver may mount directly on a duct, as in a transmission line.

### 6.3 Three views of one solid

From the same parameters derive:

1. The air network (volumes, lengths, areas) for `lumped` and `waveguide`, computed by formula in the engine.
2. The print solid, built on the web side with constructive solid geometry using the `manifold-3d` package, the Manifold kernel compiled to WebAssembly. It guarantees manifold output. Run it in a worker. Call its `setup()` after loading, and release every object it creates with `.delete()`, because they are not garbage collected. Cost uses the engine's own solid volume, not this solid's.
3. The voxel grid for `fdtd`, sampled in the engine directly from the primitives, or from the print solid's mesh for an outer shape that has no formula.

These must agree (CHK-GEO-1). When they do not, the geometry code is wrong.

### 6.4 Export

- Binary STL from Level 1 and 3MF from Level 2. Print files are in millimeters, so multiply by 1000 on export.
- Solids are watertight and manifold (CHK-GEO-2).
- Driver cut-outs, recesses, and bolt holes come from the driver's mounting data.
- From Level 2, split the cabinet into parts that fit the printer's build volume, add alignment features, and report part count, material mass, and print time.
- Enforce a minimum wall thickness and warn about thin features. Wall vibration is not simulated, and the interface says so.

## 7. Crossover and DSP chain

- Order for each radiator: crossover filters, per-radiator EQ, delay, gain and polarity, limiter. Blocks for the whole speaker come before the split.
- Biquads follow the RBJ Audio EQ Cookbook, with `H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)`. Types: lowpass, highpass, peaking, low shelf, high shelf, allpass, notch.
- Crossover families: Butterworth of order n, and Linkwitz–Riley of order 2n, which is two Butterworth filters of order n in cascade. LR4 and LR8 sum flat in phase. LR2 needs one side inverted. The lab sets polarity automatically and shows the summed response either way.
- Every block provides its complex frequency response for simulation and a sample-by-sample process function for audio, from one implementation.
- Delay is whole samples at Level 1 and fractional from Level 2. FIR blocks and linear-phase crossovers come at Level 3, with the added latency shown.
- Changing a parameter during playback crossfades between old and new settings over about 20 ms.
- Export for real hardware: for each radiator, a text file of biquad coefficients at a chosen sample rate, and FIR filters as mono WAV files. Hardware-specific formats wait for the owner's choice of playback hardware (`STATUS.md`).

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

C2 finds the lowest frequency `f_low` at which `max_spl(f)` still reaches the design level, shapes the low end into a clean high-pass at `f_low` (second order for a sealed box, fourth order for a vented one) by boosting within headroom, and always adds a protective high-pass below. Below a vented box's tuning the cone is unloaded, so that high-pass is not optional.

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
- Every real record has a `tier`: `budget` or `reference`. Budget parts are the point of the lab: inexpensive drivers, full-range units, amplifier boards, and DSP options sold new today. Reference parts are a small set of well-regarded quality components, kept so that designs can be compared with a conventional build. When choosing what to add, budget parts come first.
- Only new, off-the-shelf parts with published data. Used, salvaged, or self-measured parts are not catalog records.
- `synthetic` records exist for checks and demos. Their ids start with `test-`. The interface shows them as "Test driver, not a real product". The optimizer never proposes them.
- Prices go stale. Show `read_on` beside every price, and warn when a price is more than 90 days old. Prices are in USD.
- A figure read anywhere other than the part's own page, such as a search result's extract or another product's listing, carries `"via"` naming where it came from (`"search_extract"` or `"other_listing"`). The interface shows it as unconfirmed, and a cost that includes it says so. Replace it, and drop `via`, when the part's own page opens.

Amplifiers and DSP devices in `catalog/electronics/` follow the same sourcing rules. An amplifier record holds channel count, power per channel at a rated load, and price. A DSP device record holds output channels, sample rates, filter capacity, and price. The synthetic amplifier `test-amp-a` has one channel of 50 W into 4 ohm.

Exciters, from Level 3, are records like drivers. They hold what an exciter's data sheet prints: coil resistance and inductance, force factor, coil and magnet mass, suspension stiffness and loss, travel, power, and mounting footprint. Panel materials, also from Level 3, hold `E`, density, Poisson's ratio, and loss factor, each with a source or a note of how it was measured.

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

- The AudioWorklet runs the design's own DSP chain in the `seatlock-rt` kernel and outputs one channel per radiator. This is the part that would run on real hardware.
- The virtual speaker follows it. Each radiator's channel is convolved with that radiator's simulated impulse response, from terminal voltage to pressure at the listening position, and the results are summed. The engine computes the impulse responses. Use one `ConvolverNode` per radiator with `normalize = false`, because the default rescales the response.
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

From Level 3 the results also include sound power, a directivity index, and, when the design has a room, the in-room response of section 5.10. A design with a room is scored on its in-room response, using the target's tilt. A design without one is scored on its direct response.

### 11.2 Targets

A target is a set of thresholds in `targets/`. A design meets the target when it satisfies all of them.

```jsonc
// targets/hifi-default.json
{ "f6_hz_max": 50, "flatness_db_max": 1.5, "max_spl_db_min": 95, "budget_max": null }
```

These numbers are placeholders chosen when this specification was written. They are not the owner's decision yet. Ask the owner before Level 2 optimizer work depends on them (`STATUS.md`).

Among designs that meet the target, the cheaper one is better. Among designs that miss it, the one with the smaller shortfall is better:

```
shortfall = max(0, log2(f6_hz / f6_hz_max))
          + max(0, flatness_db - flatness_db_max) / 1.5
          + max(0, max_spl_db_min - max_spl_db) / 6
```

### 11.3 Optimizer

- It answers one question: what is the cheapest design that meets this target?
- Level 2 variables, for drivers the user has chosen: sealed or vented, box volume, tuning, crossover frequencies and slopes, driver gains. Correction runs inside every evaluation.
- Level 3 adds the choice of drivers from the catalog, cabinet proportions, and internal elements.
- Method: for each combination of discrete choices, search the continuous variables with a derivative-free method on the `lumped` or `waveguide` model. Keep the best few. Verify the finalists with `fdtd` and report what changed.
- Output: a ranked list and a cost-against-quality plot. Every result is a complete design file the user can open.
- It runs in workers, shows first results within 30 s, can be canceled, and is repeatable from its seed.

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
- F: Butterworth and Linkwitz–Riley crossovers, fractional delay, automatic polarity, per-radiator limiter. CHK-DSP-2.
- G: C3 and C4; EQ per radiator and for the whole speaker.
- H: all radiators playing; A/B between two designs at matched level; live limit indicators.
- I: optimizer over continuous variables for chosen drivers; cost-against-quality plot; cost beside every score; the cheap-against-reference comparison of section 9.2.
- End to end, CHK-E2E-2: the example design `two-way-vented` passes the same sequence as CHK-E2E-1, and its three enclosure models agree within 1.5 dB below 200 Hz.

**Level 3, novel geometry and advanced correction.**

- A: undo and redo; design history.
- B: exciters and panel materials; stale-price warnings; alternative vendors per part.
- C: folded and tapered lines, coupled chambers, drivers mounted on ducts, damping fill, several drivers sharing a chamber, passive radiators; the `dml_panel` radiator with open and sealed backing; the `simple` room. CHK-PLATE-1, CHK-ROOM-1.
- D: any internal geometry the builder can make; fill and wall losses; the time budgets of section 5.5 met; `exterior` radiation for any outer shape and any facing.
- E: editor for ducts with bends and tapers, partitions, braces, and fill regions; printability warnings; `revolved` and `loft` outer shapes; mounts on any surface.
- F: FIR blocks and linear-phase crossovers, with latency shown.
- G: C5, C6, C7, C10.
- H: audible limit behavior and the room estimate in the virtual speaker.
- I: optimizer chooses drivers, build method, and stock materials from the catalog as well as geometry and DSP; the where-the-money-matters view of section 9.2; finalists verified with `fdtd`; designs with a room scored on the in-room response.
- End to end, CHK-E2E-3: the example design `folded-line` runs in all three enclosure models, and the report lists what each model ignored. The example design `rear-panel`, a rear-facing DML panel on an outer shape that is not a box, runs with `exterior` radiation and the `simple` room, and its in-room response differs from its direct response.

**Level 4, closing the loop with real speakers.** Import measurements (FRD and ZMA text files, impulse responses), fit the model to a measured impedance curve, C9, C8, export for the owner's chosen playback hardware, imported meshes as outer shapes, and panels that open into a cabinet chamber. Plan this level with the owner when Level 3 is done.

## 13. Checks

Each check becomes an automated test named after its id. The numbers were produced on 2026-11-09 by a separate reference implementation of the equations in section 5, including a small 3D solver. Identity checks (CHK-DRV-2, CHK-BOX-1, CHK-BOX-3, CHK-DSP-2) compare the implementation with closed-form results that are known independently of this code. The value checks anchor your implementation to the same equations. Stored expected values in the end-to-end checks catch regressions only. They never prove correctness.

Test parts:

```
test-woofer-a   fs 50 Hz, Qes 0.45,   Qms 4.0, Vas 10 L, Re 6.0 ohm, Le 0.5 mH, Sd 85 cm^2,  Xmax 4 mm, Pe 30 W
test-woofer-b   fs 40 Hz, Qes 0.4144, Qms 5.0, Vas 20 L, Re 6.0 ohm, Le 0,      Sd 130 cm^2, Xmax 6 mm, Pe 50 W
test-amp-a      1 channel, 50 W into 4 ohm
test-panel-a    0.40 x 0.30 m, 3 mm thick, E 3.5 GPa, 1240 kg/m^3, Poisson's ratio 0.35
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
| CHK-FDTD-4 | `fdtd` at 5 mm against `waveguide` for the CHK-BOX-3 cabinet as the builder makes it. | Tuning frequency within 3 %. SPL within 1.5 dB below 300 Hz. |
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
| CHK-PLATE-1 | `test-panel-a`, all four edges simply supported, no exciter, no loss. | The first five resonances are 42.36, 88.10, 123.68, 164.35, and 169.43 Hz. Within 1 %. |
| CHK-ROOM-1 | `simple` room of 50 m^3 with rt60 0.4 s, no wall reflection, a source radiating 1 W equally in all directions. | A = 20.12 m^2. Diffuse level 113.12 dB. Direct level at 2.5 m 101.19 dB. Within 0.05 dB. |
| CHK-E2E-1, 2, 3 | Section 12. | As stated there. |

## 14. Interface

- Plain language first. Labels use everyday words, with the technical term in the explanation, for example "Cone travel (excursion)". Section 16 lists the pairs.
- Every control and every graph has a one-sentence explanation on hover or tap.
- Every number shows its unit. Every graph shows its fidelity label. Every warning appears beside its cause and says what to do about it.
- Editing feels instant. `lumped` and `waveguide` results update within 50 ms of a change. Slower work shows progress and can be canceled. Results that no longer match the design are grayed out until they are recomputed.
- Interface work never makes the audio glitch.
- Frequency axes are logarithmic, 20 Hz to 20 kHz by default.
- The app asks for `baffle` radiation by default, because a real cabinet loses bass to the baffle step. `point` in `half` space is for checks, which name their own model.
- The screen holds design controls, the 3D view and graphs, score and cost, and a listen bar that is always within reach. Narrow screens stack them.
- A design is one file the owner can save, reopen, duplicate, and send to someone.
- A part of a kind this build does not know is shown as unsupported. It is never dropped or hidden.

## 15. Not simulated

Say so in the interface wherever it matters.

- Cabinet wall vibration. Printed walls flex, and cheap stock materials such as cardboard and foam board flex more. Until this is modeled, the builder enforces minimum wall thickness, offers bracing, and warns more strongly for materials with low stiffness.
- The listening room, beyond the simple estimate of section 5.10 that arrives at Level 3. Until then all results are for free space or a baffle.
- DML panels until Level 3. After that, a panel's result still depends on material numbers that printing makes uncertain.
- Tweeter horns and waveguides, which need a different solver.
- Voice-coil heating over time, and, until Level 3, distortion.
- Passive crossovers are out of scope by design.

## 16. Glossary

| Interface wording | Technical term | Meaning |
| --- | --- | --- |
| Driver | Loudspeaker drive unit | A single woofer, tweeter, or full-range unit. |
| Sound source | Radiator | Anything driven by one amplifier channel that makes sound: a cone driver with its enclosure, or a DML panel. |
| Panel speaker | DML panel, `dml_panel` | A stiff, light panel made to vibrate by an exciter. It radiates through many bending resonances at once. |
| Exciter | Inertial exciter | The small motor fixed to a panel that makes it vibrate. |
| Sound power | `W` | All the sound a speaker sends out, in every direction. |
| In-room response | Section 5.10 | The sound that reaches the listener directly and by way of the room. |
| Cone travel | Excursion, `x_peak`, limit `xmax_m` | How far the cone moves from rest. |
| Port air speed | Port velocity | Peak speed of air in a port. Too fast and the port makes noise. |
| Bass limit | `f6_hz` | The lowest frequency the speaker reproduces within 6 dB of its normal level. |
| Evenness | `flatness_db` | How closely the response follows the target. |
| Maximum clean level | `max_spl_db` | The loudest the speaker plays without exceeding a limit. |
| Tuning | `f_b` | The frequency at which a port takes over from the cone. |
| Crossover | Digital isolated crossover | The split of the signal between radiators, done digitally, one channel per radiator. |
| Correction | Auto EQ and the features of section 8 | Digital processing that improves the response within the hardware's limits. |
| Budget part, reference part | `tier` | A budget part is the cheap option the lab is built around. A reference part is a quality component kept for comparison. |
| Stock material | `catalog/stock/` | An off-the-shelf building material such as PVC pipe, a cardboard tube, foam board, or plywood. |
| Hybrid build | Build method | A cabinet that combines stock parts with printed parts. |
| Quick, Detailed, Full 3D | `lumped`, `waveguide`, `fdtd` | The three enclosure models, from fastest to most complete. |
| Thiele–Small parameters | `fs_hz`, `qes`, `qms`, `qts`, `vas_m3`, `re_ohm`, `le_h`, `sd_m2`, `bl`, `mms`, `cms`, `rms` | The published numbers that describe a driver: resonance frequency, electrical, mechanical, and total Q, equivalent air volume, coil resistance and inductance, cone area, force factor, moving mass, suspension compliance, mechanical resistance. |
