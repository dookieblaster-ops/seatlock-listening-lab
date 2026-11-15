# C-Note MT (kit copy)

A digital copy of the Parts Express C-Note MT bookshelf speaker kit, for comparing the lab's passive-crossover model with an independent CTA-2034 measurement.

## Credit and license

- **Kit:** Parts Express C-Note MT Bookshelf Speaker Kit (DSA135-8 + ND25FW-4, passive PCB). Assembly manual with schematic: https://www.parts-express.com/pedocs/300-7140--c-note-assembly-manual-with-crossover-pcbs.pdf (read 2026-11-15).
- **Measurement:** Audio Science Review, "Parts Express DIY C-Note Speaker Review", https://www.audiosciencereview.com/forum/index.php?threads/parts-express-diy-c-note-speaker-review.12693/. CTA-2034 curves are in `asr/` (from https://www.spinorama.org/speakers/Part%20Express%20DIY%20C-Note/ASR/index_asr.html, CC BY-NC-SA 4.0).

## Files

| File | What it is |
| --- | --- |
| `c-note-mt.seatlock.json` | Lab design: 11×7.5×9.5 in box, 1/2 in walls, rear port, kit passive netlist. |
| `asr/cea2034.json` | ASR on-axis, listening window, early reflections, and sound power (relative dB). |
| `asr/README.md` | Credit, license, and what the comparison shows. |

## What the design says

- Drivers: Dayton DSA135-8 (catalog) and ND25FW-4 (catalog; Q from the maker's DATS file in the FRD/ZMA package because the sheet prints Q as N/A; Sd from the printed 1 in voice-coil diameter because the sheet prints Sd as N/A).
- Crossover: kit PCB schematic — tweeter R1 6 Ω → C1 5.1 µF → (L2 0.25 mH shunt) → C2 2 µF → (C3 2 µF shunt); woofer L1 1.4 mH → (C4 6.8 µF shunt). Normal polarity. Ideal inductors (no DCR). Optional 0.22 µF across L1 not fitted.
- Cabinet: outer 279.4 × 190.5 × 241.3 mm, wall 12.7 mm (`box`, not `sheet_box`: the catalog's MDF sheet is 3/4 in). Port 34.925 mm ID × 177.8 mm on the back. Mount centers estimated from the undimensioned front drawing.
- Correction off; one amplifier; listen mode still plays the (empty) digital chain.

## Not in the model

- Waveguide directivity of the ND25FW-4 (on-axis FRD is used; side angles still use the piston model).
- 1/2 in MDF as vibrating stock sheet (parametric box walls).
- Exact baffle hole coordinates from a dimensioned drawing.
- Inductor winding resistance; stuffing; rounded front vertical edges from the kit drawing.
