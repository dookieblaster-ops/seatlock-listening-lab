import { describe, expect, it } from "vitest";
import type { WaveFieldReport } from "./WaveView";

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
