import { expect, test, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

/** CHK-GEO-2 on the exported STL: closed, outward, millimeters, same volume as the solid. */
async function checkExportedStl(page: Page) {
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export print file" }).click();
  const download = await downloadPromise;
  const file = await download.path();
  const bytes = fs.readFileSync(file!);
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
  expect(maxCoord).toBeGreaterThan(10);
  expect(maxCoord).toBeLessThan(1000);
  // One assertion for all edges: an expect() per edge takes seconds on a large mesh.
  const unshared = [...edges.values()].filter((count) => count !== 2).length;
  expect(unshared).toBe(0);
  const solidM3 = Number(await page.getByTestId("solid-volume").innerText());
  expect(Math.abs(signed / 1e9 - solidM3) / solidM3).toBeLessThan(0.001);
}

/** CHK-GEO-1: the engine's formula, the printed solid, and the 5 mm voxels agree. */
async function checkVolumes(page: Page, segments: boolean) {
  const formulaSolid = Number(await page.getByTestId("formula-solid").innerText());
  const printed = Number(await page.getByTestId("solid-volume").innerText());
  // A round part is printed as a 96-sided polygon, which holds about 0.07 % less than the circle.
  expect(Math.abs(printed - formulaSolid) / formulaSolid).toBeLessThan(segments ? 0.005 : 0.001);
  const air = Number(await page.getByTestId("formula-air").innerText());
  const voxels = Number(await page.getByTestId("voxel-air").innerText());
  expect(Math.abs(voxels - air) / air).toBeLessThan(0.02);
}

test("print solid matches the air volume and exports a solid STL", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("load-example").click();
  const formula = page.getByTestId("formula-air");
  const printed = page.getByTestId("print-air");
  await expect(formula).toBeVisible({ timeout: 20_000 });
  const formulaValue = Number(await formula.innerText());
  const printedValue = Number(await printed.innerText());
  expect(Math.abs(printedValue - formulaValue) / formulaValue).toBeLessThan(0.005);
  await checkVolumes(page, true);
  await checkExportedStl(page);
});

test("a PVC tube prints two end caps and lists its pipe cut", async ({ page }) => {
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/pvc-tube.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
  await checkVolumes(page, true);
  await expect(page.getByTestId("cut-list")).toContainText("0.500 m");
  await expect(page.getByTestId("cost-note")).toContainText("for a stereo pair");
  await checkExportedStl(page);
});

test("an MDF box prints nothing, lists its panels, and buys a whole sheet", async ({ page }) => {
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/mdf-box.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("cost-note")).toContainText("for a stereo pair", { timeout: 20_000 });
  await expect(page.getByTestId("cost-note")).toContainText("unconfirmed");
  await expect(page.getByTestId("cut-list")).toContainText("sheet 1");
  await expect(page.getByTestId("cut-list")).toContainText("front 1 250 x 160 mm");
  await expect(page.getByRole("button", { name: "Export print file" })).toBeDisabled();
  await expect(page.getByTestId("scores")).toBeVisible();
});

for (const name of ["rounded-box", "chamfered-box", "sloped-baffle", "hex-prism", "two-way-vented", "two-chamber", "folded-line", "shaped-box"]) {
  test(`${name}: print solid, voxels, and formula agree, and the STL is solid`, async ({ page }) => {
    await page.goto("/");
    const example = path.resolve(import.meta.dirname, `../../designs/examples/${name}.seatlock.json`);
    await page.getByTestId("open-design-input").setInputFiles(example);
    await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
    const formula = Number(await page.getByTestId("formula-air").innerText());
    const printed = Number(await page.getByTestId("print-air").innerText());
    expect(Math.abs(printed - formula) / formula).toBeLessThan(0.005);
    await checkVolumes(page, true);
    await checkExportedStl(page);
    await expect(page.getByTestId("scores")).toBeVisible();
  });
}

test("a cabinet too big for the printer is cut into parts that fit, with pins, in one 3MF", async ({ page }) => {
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/two-chamber.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  const table = page.getByTestId("print-parts");
  await expect(table).toBeVisible({ timeout: 30_000 });
  const rows = table.locator("tbody tr");
  const names = await rows.locator("td:first-child").allInnerTexts();
  const cabinetParts = names.filter((name) => name.startsWith("part"));
  const pins = names.filter((name) => name.startsWith("pin"));
  expect(cabinetParts.length).toBe(8);
  expect(pins.length).toBeGreaterThanOrEqual(24);
  for (const size of await rows.locator("td:nth-child(2)").allInnerTexts()) {
    const mm = size.replace(" mm", "").split(" x ").map(Number).sort((a, b) => a - b);
    expect(mm[0]).toBeLessThanOrEqual(256);
    expect(mm[1]).toBeLessThanOrEqual(256);
    expect(mm[2]).toBeLessThanOrEqual(260);
  }
  await expect(page.getByTestId("print-time")).toContainText("8 parts");
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export 3MF" }).click();
  const download = await downloadPromise;
  const text = fs.readFileSync((await download.path())!).toString("latin1");
  const objects = text.match(/<object id=/g) ?? [];
  expect(objects.length).toBe(names.length);
  expect(text).toContain('name="part 1"');
  expect(text).toContain('name="pin 1"');
});

for (const [name, has] of [
  ["rounded-box", "polyhedron("],
  ["hex-prism", "linear_extrude("],
  ["two-way-vented", "cylinder("],
  ["shaped-box", "part = 0;"],
]) {
  test(`${name} is saved as an OpenSCAD program of the same solids`, async ({ page }) => {
    await page.goto("/");
    const example = path.resolve(import.meta.dirname, `../../designs/examples/${name}.seatlock.json`);
    await page.getByTestId("open-design-input").setInputFiles(example);
    await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
    const downloadPromise = page.waitForEvent("download");
    await page.getByTestId("export-scad").click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe("cabinet.scad");
    const text = fs.readFileSync((await download.path())!, "utf8");
    expect(text).toContain("module cabinet()");
    expect(text).toContain("difference()");
    expect(text).toContain(has);
    expect(text.split("{").length).toBe(text.split("}").length);
    // For checking the file in OpenSCAD itself, outside this test.
    if (process.env.SCAD_OUT) {
      const solid = await page.getByTestId("formula-solid").innerText();
      fs.writeFileSync(`${process.env.SCAD_OUT}-${name}.scad`, `// lab solid: ${solid}\n${text}`);
    }
  });
}
