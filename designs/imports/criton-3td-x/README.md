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
- Cabinet: 215.9 × 1054.1 × 342.9 mm outer, 19.05 mm walls. Driver centers from the front drawing, measured down from the top (146, 267, 451, and 648 mm). The sealed top chamber's floor is the midpoint between the midrange and the upper woofer. Port 76.2 mm ID × 203.2 mm, the drawing's substitute when the kit's flared tube is not used. Port height on the back is not taken from a called-out dimension.

## Not in the model

- The kit's flared aluminum port (a straight 3 × 8 in tube stands in).
- Cabinet fill (the drawing says to line the walls).
- Maker response curves (none were published as number files in the sources opened).
- Plywood or MDF wall vibration (the box is the drawn size, not a sheet-stock cabinet).
- Exact partition and port-height dimensions.
