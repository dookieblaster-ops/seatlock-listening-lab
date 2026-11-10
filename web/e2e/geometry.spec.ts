import { expect, test } from "@playwright/test";
import fs from "node:fs";

test("print solid matches the air volume and exports a solid STL", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("load-example").click();
  const formula = page.getByTestId("formula-air");
  const printed = page.getByTestId("print-air");
  await expect(formula).toBeVisible({ timeout: 20_000 });
  const formulaValue = Number(await formula.innerText());
  const printedValue = Number(await printed.innerText());
  expect(Math.abs(printedValue - formulaValue) / formulaValue).toBeLessThan(0.005);
  const voxels = Number(await page.getByTestId("voxel-air").innerText());
  expect(Math.abs(voxels - formulaValue) / formulaValue).toBeLessThan(0.02);

  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export print file" }).click();
  const download = await downloadPromise;
  const path = await download.path();
  const bytes = fs.readFileSync(path!);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const triangles = view.getUint32(80, true);
  expect(triangles).toBeGreaterThan(10);
  const edges = new Map<string, number>();
  let volume = 0;
  let maxCoord = 0;
  for (let i = 0; i < triangles; i += 1) {
    const offset = 84 + i * 50;
    const verts: number[][] = [];
    for (let v = 0; v < 3; v += 1) {
      const x = view.getFloat32(offset + 12 + v * 12, true);
      const y = view.getFloat32(offset + 16 + v * 12, true);
      const z = view.getFloat32(offset + 20 + v * 12, true);
      maxCoord = Math.max(maxCoord, Math.abs(x), Math.abs(y), Math.abs(z));
      verts.push([x, y, z]);
    }
    volume += verts[0][0] * (verts[1][1] * verts[2][2] - verts[1][2] * verts[2][1]);
    volume += verts[0][1] * (verts[1][2] * verts[2][0] - verts[1][0] * verts[2][2]);
    volume += verts[0][2] * (verts[1][0] * verts[2][1] - verts[1][1] * verts[2][0]);
    for (let e = 0; e < 3; e += 1) {
      const a = `${verts[e].map((n) => n.toFixed(4)).join(",")}`;
      const b = `${verts[(e + 1) % 3].map((n) => n.toFixed(4)).join(",")}`;
      const key = a < b ? `${a}|${b}` : `${b}|${a}`;
      edges.set(key, (edges.get(key) ?? 0) + 1);
    }
  }
  const signed = volume / 6;
  expect(signed).toBeGreaterThan(0);
  volume = signed;
  expect(maxCoord).toBeGreaterThan(10);
  expect(maxCoord).toBeLessThan(1000);
  for (const count of edges.values()) {
    expect(count).toBe(2);
  }
  const solidM3 = Number(await page.getByTestId("solid-volume").innerText());
  expect(Math.abs(volume / 1e9 - solidM3) / solidM3).toBeLessThan(0.001);
});
