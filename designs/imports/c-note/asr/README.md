# ASR measured curves for the C-Note MT

CTA-2034 (spinorama) curves of the Parts Express DIY C-Note, measured by Audio Science Review on a Klippel Near-Field Scanner and published through the spinorama project.

## Credit and license

- **Measurement:** Audio Science Review, "Parts Express DIY C-Note Speaker Review", https://www.audiosciencereview.com/forum/index.php?threads/parts-express-diy-c-note-speaker-review.12693/
- **Published data:** https://www.spinorama.org/speakers/Part%20Express%20DIY%20C-Note/ASR/index_asr.html
- **License:** Creative Commons Attribution-NonCommercial-ShareAlike 4.0 (CC BY-NC-SA 4.0). Credit Audio Science Review; share-alike if you adapt the data.
- **Source file read:** `CEA2034.json` from the spinorama site (2026-11-15).

## Files

| File | What it is |
| --- | --- |
| `cea2034.json` | On axis, listening window, early reflections, and sound power: 181 points from about 20 Hz to 20 kHz. Levels are relative (spinorama normalises the on-axis midband). |

## What the comparison shows (2026-11-15)

Against the lab's `c-note-mt` everyday model with the kit passive network, after aligning midband means (300 Hz to 5 kHz):

- ASR on-axis bass (40 to 270 Hz) sits about 3.6 dB below its midband on this relative file; the lab about 6.3 dB below.
- Midband shape against ASR is within about 2.1 dB RMS; 2–10 kHz about 2.6 dB RMS.
- Relative −6 dB points: ASR ~74 Hz, lab ~106 Hz (bass sits lower vs midband in the lab). Absolute kit F3 ads (~43 Hz) are a different metric.

Limits recorded in the design notes and `c_note` test: estimated baffle hole positions, ideal inductors, parametric box (not vibrating 1/2 in MDF stock), ND25FW Sd from voice-coil diameter, waveguide directivity not in side angles.

Downloaded 2026-11-15.
