import { describe, expect, it } from "vitest";
import {
  pressureColor,
  robustPeak,
  signedLogMap,
  snapshotPhase,
  type WaveFieldReport,
} from "./WaveView";
import * as THREE from "three";

describe("WaveFieldReport shape", () => {
  it("accepts a minimal standing-wave payload the page draws", () => {
    const report: WaveFieldReport = {
      domain: "inside",
      frequency_hz: 400,
      field: {
        dx_m: 0.012,
        origin_m: [0, 0, 0],
        n: [4, 4, 4],
        cells: [0, 1],
        positions_m: [0.1, 0.1, 0.1, 0.2, 0.1, 0.1],
        standing: [{ frequency_hz: 400, re: [1, -0.5], im: [0, 0.2], max_abs: 1 }],
        pulse: {
          dt_s: 0.001,
          times_s: [0, 0.001],
          frames: [
            [0.2, -0.1],
            [0.1, 0.3],
          ],
          max_abs: 0.3,
        },
      },
      notes: ["test"],
      elapsed_ms: 12,
      markers_m: [0.1, 0, 0.15],
      marker_labels: ["driver"],
    };
    expect(report.field.standing[0].max_abs).toBe(1);
    expect(report.field.pulse?.frames).toHaveLength(2);
  });
});

describe("wave display helpers", () => {
  it("maps signed log without flipping signs", () => {
    expect(signedLogMap(0)).toBe(0);
    expect(signedLogMap(1)).toBeGreaterThan(0);
    expect(signedLogMap(-1)).toBeLessThan(0);
    expect(Math.abs(signedLogMap(1))).toBeLessThan(1.0001);
    // A hot outlier compresses relative to linear: mid values keep more share.
    expect(Math.abs(signedLogMap(0.2)) / Math.abs(signedLogMap(1))).toBeGreaterThan(0.2);
  });

  it("uses a robust peak below the absolute max", () => {
    const values = [0.1, 0.12, 0.11, 0.13, 0.1, 5.0];
    const peak = robustPeak(values, 0.8);
    expect(peak).toBeLessThan(5);
    expect(peak).toBeGreaterThan(0.1);
  });

  it("snaps freeze phase to a readable octant", () => {
    expect(snapshotPhase(0.1)).toBeCloseTo(0, 6);
    expect(snapshotPhase(Math.PI / 4 + 0.05)).toBeCloseTo(Math.PI / 4, 6);
  });

  it("keeps the teal↔amber diverging map (no purple hub)", () => {
    const neg = pressureColor(-1, new THREE.Color());
    const pos = pressureColor(1, new THREE.Color());
    const zero = pressureColor(0, new THREE.Color());
    expect(neg.b).toBeGreaterThan(neg.r);
    expect(pos.r).toBeGreaterThan(pos.b);
    expect(zero.r + zero.g + zero.b).toBeLessThan(1.2);
  });
});
