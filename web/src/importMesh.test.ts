import { describe, expect, it } from "vitest";
import { fromField, parseStl, partField, quarterTurn } from "./importMesh";

/** A binary STL of a cube from 10 to 30 mm, wound outward. */
function cubeStl(): ArrayBuffer {
  const c = (i: number) => [i & 1 ? 30 : 10, i & 2 ? 30 : 10, i & 4 ? 30 : 10];
  const tris = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
  const buffer = new ArrayBuffer(84 + 50 * tris.length);
  const view = new DataView(buffer);
  view.setUint32(80, tris.length, true);
  tris.forEach((t, n) => {
    t.forEach((corner, k) => c(corner).forEach((v, a) => view.setFloat32(84 + 50 * n + 12 + 12 * k + 4 * a, v, true)));
  });
  return buffer;
}

function volume(v: Float32Array, t: Uint32Array): number {
  let sum = 0;
  for (let i = 0; i < t.length; i += 3) {
    const [a, b, c] = [t[i], t[i + 1], t[i + 2]].map((j) => [v[3 * j], v[3 * j + 1], v[3 * j + 2]]);
    sum += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6;
  }
  return sum;
}

describe("importing a speaker", () => {
  it("reads a binary STL in millimeters and welds its corners", () => {
    const part = parseStl(cubeStl(), "cube");
    expect(part.vertices.length).toBe(24);
    expect(part.triangles.length).toBe(36);
    expect(volume(part.vertices, part.triangles)).toBeCloseTo(8e-6, 10);
  });

  it("keeps the volume and the winding through a quarter turn and the design file", () => {
    for (const axis of [0, 1, 2] as const) {
      const [turned] = quarterTurn([parseStl(cubeStl(), "cube")], axis);
      expect(Math.min(...turned.vertices)).toBeCloseTo(0, 9);
      const back = fromField(partField(turned));
      expect(volume(back.vertices, back.triangles)).toBeCloseTo(8e-6, 10);
    }
  });
});
