# CSS Criton 3TD-X (kit copy)

A digital copy of the Creative Sound Solutions Criton 3TD-X tower, for comparing the lab's passive-crossover model with Audio Science Review's CTA-2034 measurement.

## Credit and license

- **Kit:** CSS Criton 3TD-X, pair $1,999 on https://www.css-audio.com/products/diy-speaker-kit-criton-3td-x-speaker-pair (read 2026-11-15). Drawing (cabinet, crossover, driver outlines): https://doc.soundimports.nl/pdf/brands/CSS/Criton-3TD-X/Criton-3TD-X-datasheet.pdf dated 10 May 2024, designed by D. Poinsett / K. Armes. Assembly: https://www.css-audio.com/cdn/shop/files/css-criton-3td-x-assembly_dc082b6b-6c6e-491e-b4ca-d242fa7898eb.pdf
- **Measurement:** Audio Science Review via the spinorama project, https://www.spinorama.org/speakers/CSS%20Criton%203TD-X%20DIY/ASR/index_asr.html (CC BY-NC-SA 4.0). ASR's measurement list dates the review 2024-11-12. Preference score on that page 4.68. Curves are in `asr/`.

## Files

| File | What it is |
| --- | --- |
| `criton-3td-x.seatlock.json` | Lab design: drawn outer size, two chambers, substitute port, kit passive netlist. |
| `asr/cea2034.json` | ASR on-axis, listening window, early reflections, and sound power (relative dB). |
| `asr/README.md` | Credit and what the comparison shows. |

## What the design says

- Drivers: two CSS LDW7 woofers (parallel on one crossover branch), one LDW6 midrange, one LD25X tweeter. Parameters from SoundImports listings opened 2026-11-15; LD25X Thiele-Small from the Parts Express spec sheet the same day (sensitivity 90 dB is only on the SoundImports listing).
- Crossover from the 10 May 2024 schematic, inductor DCR included: tweeter 1.5 Ω, 5.6 µF, 400 µH (0.28 Ω) shunt, 2 Ω, 22 Ω across the tweeter. Midrange 1 Ω, 75 µF, 3 mH (1.4 Ω) shunt, 750 µH (0.286 Ω), 10 µF shunt, 3 Ω. Woofers 3 mH (0.349 Ω), then both woofers in parallel, with 125 µF in series with 1 Ω to ground.
- Cabinet, from the 10 May 2024 drawing and the assembly guide: outer 215.9 × 1054.1 × 342.9 mm, walls 19.05 mm. Driver centers down from the top are 146, 267, 451, and 648 mm. The tweeter and midrange share a sealed box on the front of the top, 6 in deep (the short side of braces C and D; 6.75 in from the outside front) and 13.5 in down from the top. That height is also what fits the two 6 × 7 × 3.5 in denim pads with the brace between them. Woofer air is the rest of the cabinet, including the space behind that box up to the top, joined through the open rear half of the floor. Port 76.2 mm ID × 203.2 mm on the back. The side view's 9 in callout is taken as the top of that 3 in port, so the center is 7.5 in up.

## Not in the model

- The kit port's flare (the drawing's 3 × 8 in tube is used; the assembly photo shows foam around it, not the flare profile).
- The denim pads and the egg-crate foam (the everyday model has no lining). Two 7 × 6 × 3.5 in denim pieces go in the top box, one behind the tweeter and one behind the midrange. Egg-crate pieces line the woofer walls above the port.
- Brace windows. The horizontal braces are full-depth plates with holes; a solid brace in the lab would block the air, so they are left out.
- The 0.47 µF capacitor in the assembly parts list. It is not readable on the schematic.
- Maker response curves (no number file in the sources opened).
- Plywood or MDF wall vibration (the box is the drawn size, not a sheet-stock cabinet).
