# LCM Nearfield Monitor (imported)

A known DIY speaker, imported from its designer's own files so that the lab's predictions can be compared with an independent measurement.

## Credit and license

- **Design:** "LCM Monitor Speaker - 40Hz waveguide - Bluetooth - DSP" by zx82net (Luke), https://www.thingiverse.com/thing:4980197.
- **License:** Creative Commons Attribution - Non-Commercial. The STL and DSP files here are the designer's, unchanged. The `.seatlock.json` designs are built from them by `build_designs.py`, so they carry the same license.
- **Build notes:** https://www.instructables.com/LCM-Waveguide-Speaker-3D-Printed-Near-field-Studio/ (published 2021-11-16, read 2026-11-13). Parts Express project gallery: https://projectgallery.parts-express.com/speaker-projects/lcm-little-californian-monitor/.
- **Measurement:** Erin's Audio Corner, "LCM Nearfield Monitor 3-D Printed DIY Speaker Kit Review", 2022-06-22, https://erinsaudiocorner.com/loudspeakers/lcm_nearfield_monitor/ (Klippel Near-Field Scanner). Its summary figures are on https://www.spinorama.org/speakers/LCM%20Nearfield%20Monitor%203-D%20Printed%20DIY/ErinsAudioCorner/index_eac.html (data CC BY-NC-SA 4.0). The curves themselves are not in this repository yet.

## Files

| File | What it is |
| --- | --- |
| `LCM_Proto_1_EVO_Case_L.stl`, `..._R.stl` | The March 2022 "EVO" cabinet, two halves, as printed (lying on their outer sides). Most likely the version Erin measured in June 2022. |
| `LCM_Printed_Case_L.stl`, `..._R.stl` | The October 2021 cabinet, two halves, assembled in place. Thingiverse's checksums match. |
| `zx82net_LCM_DSP_project_simple.dspproj` | The designer's SigmaStudio project. Read for: crossover 200 Hz on both sides, tweeter -6 dB, +2.5 dB at 10 kHz. |
| `build_designs.py` | Builds the two designs below from the STL files (`python3 build_designs.py`). |
| `lcm-evo.seatlock.json`, `lcm-2021.seatlock.json` | The lab's designs: the imported cabinet with its holes marked, the two drivers, the designer's DSP. |

All downloaded 2026-11-13.

## What the design says

- Drivers: Tang Band W3-1876S (3 in. woofer) and Tectonic TEBM35C10-4 (35 mm BMR), both in the catalog from their makers' sheets.
- DSP: Linkwitz-Riley 48 dB/octave at 200 Hz; tweeter -6 dB; high shelf +2.5 dB at 10 kHz (slope 1.0: the project's own slope was not decoded). Correction is off, so the lab plays the designer's settings only.
- Holes marked on the model: the woofer and tweeter holes (filled by the drivers), the two wire holes in the back (sealed in the build notes), and the port's slot mouth (open).
- Build: PLA at 30 % infill, the designer's stated minimum.

## Not in the model

- The polyfill the build notes put in the tweeter's chamber (the quick and detailed models do not model fill).
- Wall vibration (imported cabinets keep rigid walls).
- The BMR's real bending behaviour above its piston range: there is no measured response file for it yet.
