"""Builds the LCM Nearfield Monitor designs from the designer's own files.

Source: "LCM Monitor Speaker - 40Hz waveguide - Bluetooth - DSP" by zx82net (Luke),
https://www.thingiverse.com/thing:4980197, licensed Creative Commons Attribution -
Non-Commercial. Files downloaded 2026-11-13. Build notes and DSP settings:
https://www.instructables.com/LCM-Waveguide-Speaker-3D-Printed-Near-field-Studio/
(published 2021-11-16, read 2026-11-13).

Run from this folder with:  python3 build_designs.py
Writes lcm-evo.seatlock.json (the March 2022 "EVO" case) and lcm-2021.seatlock.json
(the October 2021 printed case).

Placement in the lab's axes (README section 4): x left to right facing the front, y front to
back, z up, origin at the outer front-bottom-left corner.
- EVO halves are stored lying on their outer side walls. The right half is turned over
  (180 degrees about the depth axis, then raised by 140 mm) onto the left half; the tongue
  then sits in the groove. In those coordinates the speaker's height runs along -x, so the
  lab takes x_lab = z, y_lab = y, z_lab = 27 - x (a rotation, not a mirror).
- The 2021 halves are stored assembled, height along z: x_lab = x + 70.

Holes in the model, found from the files (front face, y = 0): tweeter 46 mm round, centered
156 mm (EVO) or 153 mm (2021) up; woofer 93 mm round recess and cut-out, centered 71.5 mm
(EVO) or 73 mm (2021) up; port mouth a 10 x 88 mm slot, 10 mm up. All centered across.
Back face (y = 172 mm): two wire holes about 4 x 7 mm, 151.5 and 166.5 mm up, 8.5 mm (EVO)
or 6.5 mm (2021) from the left; the build notes seal them.
"""

import base64
import json
import struct
import sys

import numpy as np


def read_stl(path):
    data = open(path, "rb").read()
    count = struct.unpack("<I", data[80:84])[0]
    rows = np.frombuffer(
        data[84 : 84 + 50 * count],
        dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]),
    )
    return rows["v"].astype(np.float64)  # (count, 3, 3), mm


def place(tris, matrix, offset):
    return tris @ np.asarray(matrix, float).T + np.asarray(offset, float)


def weld(tris):
    """Merges corners at the same place to a micrometer, as the page's importer does."""
    flat = tris.reshape(-1, 3) * 0.001  # m
    keys = np.round(flat * 1e6).astype(np.int64)
    unique, inverse = np.unique(keys, axis=0, return_inverse=True)
    vertices = np.zeros((len(unique), 3))
    vertices[inverse] = flat
    triangles = inverse.reshape(-1, 3).astype(np.uint32)
    keep = (
        (triangles[:, 0] != triangles[:, 1])
        & (triangles[:, 1] != triangles[:, 2])
        & (triangles[:, 0] != triangles[:, 2])
    )
    return vertices, triangles[keep]


def part(name, tris):
    vertices, triangles = weld(tris)
    return {
        "name": name,
        "role": "solid",
        "material": "pla",
        "vertices_m": base64.b64encode(vertices.astype("<f4").tobytes()).decode(),
        "triangles": base64.b64encode(triangles.astype("<u4").tobytes()).decode(),
    }


def point(pid, at_mm, hole, back=False):
    normal = [0.0, 1.0, 0.0] if back else [0.0, -1.0, 0.0]
    return {
        "id": pid,
        "point_m": [at_mm[0] / 1000, 0.172 if back else 0.0, at_mm[1] / 1000],
        "normal": normal,
        "base_normal": normal,
        "tilt_deg": 0.0,
        "turn_deg": 0.0,
        "hole": hole,
    }


def design(name, parts, woofer_z, tweeter_z, wire_x, note):
    return {
        "schema_version": 1,
        "name": name,
        "notes": note,
        "speaker": {
            "enclosure": {
                "outer": {
                    "kind": "mesh",
                    "parts": parts,
                    "points": [
                        point("woofer-hole", (70.0, woofer_z), "seal"),
                        point("tweeter-hole", (70.0, tweeter_z), "seal"),
                        point("port-mouth", (70.0, 10.0), "opening"),
                        # The two speaker-wire holes in the back, sealed in the build notes.
                        point("wire-hole-1", (wire_x, 151.5), "seal", back=True),
                        point("wire-hole-2", (wire_x, 166.5), "seal", back=True),
                    ],
                },
                "chambers": [],
                "ducts": [],
                "mounts": [
                    {"id": "woofer", "surface": "woofer-hole", "at_m": [0.0, 0.0],
                     "rear_into": "main", "recess_m": 0.0},
                    {"id": "tweeter", "surface": "tweeter-hole", "at_m": [0.0, 0.0],
                     "rear_into": "main", "recess_m": 0.0},
                ],
                "solids": [],
            },
            "radiators": [
                {"kind": "cone", "driver_id": "tangband-w3-1876s", "mount": "woofer"},
                {"kind": "cone", "driver_id": "tectonic-tebm35c10-4", "mount": "tweeter"},
            ],
            "listening": {"distance_m": 1.0},
        },
        "dsp": {
            "sample_rate_hz": 48000,
            "whole_speaker": [],
            # The designer's SigmaStudio project (zx82net_LCM_DSP_project_simple.dspproj) and
            # build notes: Linkwitz-Riley 48 dB/octave at 200 Hz, tweeter -6 dB, +2.5 dB high
            # shelf at 10 kHz. The shelf's slope is not decoded from the project; 1.0 is used.
            "per_radiator": [
                {"blocks": [{"kind": "crossover", "family": "linkwitz-riley", "order": 8,
                             "f_hz": 200.0, "band": "low"}],
                 "delay_s": 0.0, "gain_db": 0.0, "invert": False, "limiter": None},
                {"blocks": [{"kind": "crossover", "family": "linkwitz-riley", "order": 8,
                             "f_hz": 200.0, "band": "high"},
                            {"kind": "biquad", "type": "highshelf", "f_hz": 10000.0,
                             "q": 1.0, "gain_db": 2.5}],
                 "delay_s": 0.0, "gain_db": -6.0, "invert": False, "limiter": None},
            ],
            "correction": {"enabled": False, "protected_band_hz": [3000.0, 5000.0],
                           "max_filters": 10},
        },
        "electronics": {"amplifiers": ["dayton-kabd-4100"], "dsp_device": None},
        "build": {"material": "pla", "infill_fraction": 0.3, "stereo_pair": True,
                  "extras_cost": 0.0},
        "target": "hifi-default",
    }


CREDIT = (
    "LCM Nearfield Monitor by zx82net (Luke), from the designer's own model files "
    "(thingiverse.com/thing:4980197, CC BY-NC). Infill 30 % is the designer's stated minimum."
)


def main():
    here = sys.argv[1] if len(sys.argv) > 1 else "."
    left = read_stl(f"{here}/LCM_Proto_1_EVO_Case_L.stl")
    right = read_stl(f"{here}/LCM_Proto_1_EVO_Case_R.stl")
    turn = [[-1, 0, 0], [0, 1, 0], [0, 0, -1]]
    right = place(right, turn, [0, 0, 140])
    to_lab = [[0, 0, 1], [0, 1, 0], [-1, 0, 0]]  # x_lab = z, y_lab = y, z_lab = -x (+27)
    left = place(left, to_lab, [0, 0, 27])
    right = place(right, to_lab, [0, 0, 27])
    evo = design("lcm-evo", [part("case, left half", left), part("case, right half", right)],
                 71.5, 156.0, 8.5, CREDIT + " EVO case, March 2022.")
    json.dump(evo, open(f"{here}/lcm-evo.seatlock.json", "w"))

    left = read_stl(f"{here}/LCM_Printed_Case_L.stl")
    right = read_stl(f"{here}/LCM_Printed_Case_R.stl")
    left = place(left, np.eye(3), [70, 0, 0])
    right = place(right, np.eye(3), [70, 0, 0])
    old = design("lcm-2021", [part("case, left half", left), part("case, right half", right)],
                 73.0, 153.0, 6.5, CREDIT + " Printed case, October 2021.")
    json.dump(old, open(f"{here}/lcm-2021.seatlock.json", "w"))
    for tag, tris in (("evo", np.concatenate([left, right])),):
        lo = tris.reshape(-1, 3).min(0)
        hi = tris.reshape(-1, 3).max(0)
        print(tag, "2021 bounds mm", lo.round(2), hi.round(2))


if __name__ == "__main__":
    main()
