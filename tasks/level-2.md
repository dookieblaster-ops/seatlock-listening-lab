# Task: Level 2, a two-way vented speaker from real parts

Read README.md, STATUS.md, and COMS.md first. README section 12 lists Level 2; this file says how to build it and in what order.

Same rules as `tasks/level-1.md`: one milestone per assignment, `./check.sh` passing, a handoff in `COMS.md`, then wait for review. Every check id becomes a test named after it. For every new check, prove it fails before the code it tests exists or is fixed. No new engine dependency without asking, except those `tasks/level-1.md` already allows.

## M1. Two radiators, a port, and the crossover

Goal: a two-way vented speaker simulates, plays, and compares against another design, with every track connected.

1. **Several radiators.** Everything that handles one radiator handles a list: `simulate`, the graphs (one curve per radiator plus the sum), correction, cost, the listen chain (one kernel channel and one convolver per radiator), and the form.
2. **Vented `lumped` box (README section 5.3).** The design describes the port as a `duct` from `main` to `outside` (README section 6.2). The `lumped` model treats a straight duct as a port with the end corrections of section 5.3. The opening is a second surface source with `U_s = -H_port * U_d` (section 5.6). Port air speed goes into results and limits. Checks: CHK-BOX-3 and CHK-BOX-4, through `simulate` as well as directly.
3. **Crossovers (README section 7).**
   - Butterworth of order 1 to 4, and Linkwitz–Riley 2, 4, and 8, as blocks.
   - Automatic polarity: when the summed response with the higher branch inverted is flatter through the crossover region, invert it and say so.
   - Fractional delay with a Thiran allpass or a short windowed-sinc FIR. Record the choice.
   - A per-radiator peak limiter whose threshold is the amplifier voltage limit.
   - Check: CHK-DSP-2.
4. **C2 for vented boxes.** Fourth-order target shape (README section 8). Below tuning, the protective high-pass is mandatory.
5. **Test parts.** Add `test-tweeter-a` (README section 13) as a synthetic driver with `Z_ab = 0` and no openings.
6. **Example design.** `designs/examples/two-way-vented.seatlock.json`: test-woofer-b in a vented box tuned near 40 Hz, test-tweeter-a above it, and an LR4 crossover at 2.5 kHz. Store its canonical form.
7. **Side by side.** Two designs loaded at once. Graphs overlay both, each score shows for both, and listen mode switches A/B with a short crossfade at matched level (equal mean level from 300 Hz to 3 kHz). Live limit indicators show per radiator.
8. **Level 1 leftovers.**
   - Correction notes report only regions at or above `f_low`, at most three, plus a count of the rest.
   - The bass sentence compares like with like: `f6_hz` before and after.
   - Correction's target includes the design's own fixed blocks, such as sealed-test's 35 Hz high-pass, so that C1 does not chase them.

## M2. Waveguide, baffle, and real drivers

1. **`waveguide` (README section 5.4).** Ducts with tapers sliced into segments of at most 10 mm, chambers as shunt compliances or as ducts, end corrections, and radiation impedances. Tree-shaped networks. Checks: CHK-WG-1, CHK-WG-2, CHK-WG-3.
2. **`baffle` radiation (README section 5.6).** Each source at its real position, piston directivity, and the edge diffraction model, with the step shelf removed in this mode. Checks: CHK-RAD-1 and CHK-RAD-2. C3 (README section 8) uses this model's response to compensate the baffle step.
3. **C4.** Per-radiator delay from the source positions, and phase alignment through the crossover: choose a delay and polarity that make the summed response flattest in the crossover region.
4. **Published response curves.** A driver record may name a response file (frequency, dB, and optionally phase). Above a blend frequency chosen per driver (default: where `k*a = 1`), the simulated response takes the measured curve's shape. Label that region `measured` in the fidelity label. Without a file, label it as described in README section 5.6.
5. **Sourced catalog (README section 9.1).** At least twelve real drivers across woofers, tweeters, and full-range units, with mounting data. Also at least two amplifiers and one DSP device. Each record is written with its data sheet or vendor page open, and its source and price carry a `read_on` date. Do not add a part whose source you cannot open; list such gaps in STATUS. Cost covers electronics and stereo pairs.

## M3. Cabinet geometry and the 3D solver with openings

1. **Outer shapes.** Rounded and chamfered edges, a sloped baffle, and `prism` (README section 6.2), each registered as its own member.
2. **Inside.** Round and slot ports, several chambers, recesses, and bolt holes from mounting data.
3. **Print parts.** Split the cabinet into parts that fit the printer's build volume, with alignment features. Export 3MF and report the print time. The printer's build volume and flow rate come from the owner. Until they are in STATUS, use a labeled placeholder of 220 x 220 x 250 mm.
4. **`fdtd` with openings (README section 5.5).** An exterior air block with absorbing faces, `H_k` from the flow through each opening, and a two-resolution uncertainty. Background jobs with progress and cancel. Checks: CHK-FDTD-3 and CHK-FDTD-4. Check the exterior block size rule of section 5.5 and record the result.

## M4. Optimizer and the end-to-end check

1. **Optimizer (README section 11.3).** Over the continuous variables for the drivers the user chose: sealed or vented, box volume, tuning, crossover frequency and slope, and gains. Correction runs inside each evaluation, on `lumped` or `waveguide`. Results show within 30 s, can be canceled, and repeat from their seed. Output is a ranked list of complete designs and a cost-against-quality plot.
2. **CHK-E2E-2 (README section 12).** `two-way-vented` passes the CHK-E2E-1 sequence, and its three enclosure models agree within 1.5 dB below 200 Hz.
