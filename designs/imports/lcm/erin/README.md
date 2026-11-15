# Erin's measured curves for the LCM

CTA-2034 (spinorama) curves of the LCM Nearfield Monitor, measured by Erin's Audio Corner on a Klippel Near-Field Scanner and published through the spinorama project.

## Credit and license

- **Measurement:** Erin's Audio Corner, "LCM Nearfield Monitor 3-D Printed DIY Speaker Kit Review", 2022-06-22, https://erinsaudiocorner.com/loudspeakers/lcm_nearfield_monitor/
- **Published data:** https://www.spinorama.org/speakers/LCM%20Nearfield%20Monitor%203-D%20Printed%20DIY/ErinsAudioCorner/index_eac.html
- **License:** Creative Commons Attribution-NonCommercial-ShareAlike 4.0 (CC BY-NC-SA 4.0). Credit Erin's Audio Corner; share-alike if you adapt the data.
- **Source file read:** `CEA2034.json` from the spinorama site (the project's GitHub repository was not reachable from this workspace on 2026-11-14; the site hosts the same published curves). Owner approved keeping a copy here on 2026-11-14 (STATUS question 11).

## Files

| File | What it is |
| --- | --- |
| `cea2034.json` | On axis, listening window, early reflections, and sound power: 194 points from about 20 Hz to 20 kHz. Levels are relative (spinorama normalises the on-axis midband mean to about 0.3 dB). Absolute sensitivity is stated as 91.4 dB at 2.83 V / 1 m (300 Hz to 3 kHz). |

## What the comparison shows (2026-11-14)

Against the lab's `lcm-evo` everyday model, after aligning midband means (300 Hz to 5 kHz):

- Erin's on-axis bass (40 to 270 Hz) is about level with its midband (+0.3 dB).
- The lab's is about 2.3 dB low there. Midband shape against Erin is within about 1.6 dB RMS; above 2 kHz about 2.2 dB RMS.
- Bass-to-midrange step (2026-11-15): with Erin's DATS T/S and free-air ~77 dB scale on the BMR (STATUS question 12), lab bass-mid is about +2.4 dB vs Erin's +0.3; the BMR band sits about 2.1 dB below the woofer's. Cause placed: that ~4 dB free-air pull-down is more than the ~1.7 dB the LCM relative shape needs — his drive-unit "about 77 dB" is a test-baffle band average, not half-space η0; the Tang Band W3-1876S has no independent acoustic measurement.

Downloaded 2026-11-14.
