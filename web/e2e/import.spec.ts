import path from "node:path";
import { expect, test } from "@playwright/test";

// Importing a speaker drawn elsewhere (README 6.2, outer shape `mesh`): parts from STL
// files, a part set as damping, a driver placed by clicking the model, and the print file.

/** A binary STL holding closed boxes in millimeters: outward boxes, or inward for a void. */
function stl(boxes: Array<{ lo: number[]; hi: number[]; out: boolean }>): Buffer {
  const order = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
  const buffer = Buffer.alloc(84 + 50 * 12 * boxes.length);
  buffer.writeUInt32LE(12 * boxes.length, 80);
  let at = 84;
  for (const { lo, hi, out } of boxes) {
    const corner = (i: number) => [i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]];
    for (const tri of order) {
      const t = out ? tri : [tri[0], tri[2], tri[1]];
      t.forEach((c, k) => corner(c).forEach((v, a) => buffer.writeFloatLE(v, at + 12 + 12 * k + 4 * a)));
      at += 50;
    }
  }
  return buffer;
}

test("an imported speaker takes a damping part and a driver placed by clicking, and prints", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });

  // The sealed-test box as a shell 160 x 180 x 250 mm with 8 mm walls, and a block of wool.
  const shell = stl([
    { lo: [0, 0, 0], hi: [160, 180, 250], out: true },
    { lo: [8, 8, 8], hi: [152, 172, 242], out: false },
  ]);
  const wool = stl([{ lo: [20, 100, 20], hi: [140, 165, 120], out: true }]);
  await page.getByTestId("import-file").setInputFiles([
    { name: "shell.stl", mimeType: "model/stl", buffer: shell },
    { name: "wool.stl", mimeType: "model/stl", buffer: wool },
  ]);
  await expect(page.getByTestId("import-message")).toContainText("Imported 2 parts");
  await expect(page.getByTestId("imported-parts")).toContainText("wool");
  await page.getByTestId("part-role-1").selectOption("damping");
  await page.getByTestId("part-resistivity-1").fill("10000");

  // Place the driver: click the middle of the model, which is its front from this view.
  await page.getByTestId("place-mount-0").click();
  const view = page.getByTestId("cabinet-view").locator("canvas");
  const box = (await view.boundingBox())!;
  await view.click({ position: { x: box.width * 0.45, y: box.height * 0.55 } });
  await expect(page.getByTestId("imported-cabinet")).toContainText("Sound source 1 (test-woofer-a): at", { timeout: 15_000 });
  await expect(page.getByTestId("response-graph-fidelity")).toContainText("Damping parts are modeled only by the full 3D model", { timeout: 30_000 });
  await expect(page.getByRole("cell", { name: /^Damping, wool/ })).toBeVisible();

  // The print: the shell with the driver's hole, closed and outward (CHK-GEO-2's edge test).
  await expect(page.getByRole("button", { name: "Export print file" })).toBeEnabled({ timeout: 30_000 });
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export print file" }).click();
  const download = await downloadPromise;
  const fs = await import("node:fs");
  const bytes = fs.readFileSync((await download.path())!);
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const triangles = dv.getUint32(80, true);
  const edges = new Map<string, number>();
  for (let i = 0; i < triangles; i += 1) {
    const verts = [0, 1, 2].map((v) => [0, 1, 2].map((a) => dv.getFloat32(84 + i * 50 + 12 + v * 12 + a * 4, true).toFixed(4)).join(","));
    for (let e = 0; e < 3; e += 1) {
      const [a, b] = [verts[e], verts[(e + 1) % 3]];
      const key = a < b ? `${a}|${b}` : `${b}|${a}`;
      edges.set(key, (edges.get(key) ?? 0) + 1);
    }
  }
  expect([...edges.values()].filter((count) => count !== 2).length).toBe(0);
  const solidM3 = Number(await page.getByTestId("solid-volume").innerText());
  // The shell less one driver hole: between the shell with no hole and that less 2 x 118 mm.
  const shellM3 = 0.16 * 0.18 * 0.25 - 0.144 * 0.164 * 0.234;
  expect(solidM3).toBeLessThan(shellM3);
  expect(solidM3).toBeGreaterThan(shellM3 - 2 * Math.PI * 0.059 * 0.059 * 0.008);
});

/** A zip archive with stored (uncompressed) entries. The importer does not check CRCs. */
function zip(files: Record<string, string>): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, text] of Object.entries(files)) {
    const nameBytes = Buffer.from(name);
    const data = Buffer.from(text);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += 30 + nameBytes.length + data.length;
  }
  const dir = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(files).length, 8);
  end.writeUInt16LE(Object.keys(files).length, 10);
  end.writeUInt32LE(dir.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, dir, end]);
}

/** 3MF mesh XML for boxes, as `stl` above orders them. */
function meshXml(boxes: Array<{ lo: number[]; hi: number[]; out: boolean }>): string {
  const order = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
  let vertices = "";
  let triangles = "";
  boxes.forEach(({ lo, hi, out }, b) => {
    for (let i = 0; i < 8; i += 1) {
      vertices += `<vertex x="${i & 1 ? hi[0] : lo[0]}" y="${i & 2 ? hi[1] : lo[1]}" z="${i & 4 ? hi[2] : lo[2]}"/>`;
    }
    for (const tri of order) {
      const t = (out ? tri : [tri[0], tri[2], tri[1]]).map((c) => c + 8 * b);
      triangles += `<triangle v1="${t[0]}" v2="${t[1]}" v3="${t[2]}"/>`;
    }
  });
  return `<mesh><vertices>${vertices}</vertices><triangles>${triangles}</triangles></mesh>`;
}

test("marking a hole on an imported model stores the seal on the placed point", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });

  const shell = stl([
    { lo: [0, 0, 0], hi: [160, 180, 250], out: true },
    { lo: [8, 8, 8], hi: [152, 172, 242], out: false },
  ]);
  await page.getByTestId("import-file").setInputFiles([{ name: "shell.stl", mimeType: "model/stl", buffer: shell }]);
  await expect(page.getByTestId("import-message")).toContainText("Imported 1 part");
  await expect(page.getByTestId("place-as-hole")).toBeVisible();
  // Place the driver as a lab-cut hole first (this shell has no cut-out), then mark it.
  await page.getByTestId("place-mount-0").click();
  const view = page.getByTestId("cabinet-view").locator("canvas");
  const box = (await view.boundingBox())!;
  await view.click({ position: { x: box.width * 0.45, y: box.height * 0.55 } });
  await expect(page.getByTestId("imported-cabinet")).toContainText("Sound source 1 (test-woofer-a): at", { timeout: 15_000 });
  await page.getByTestId("mount-hole-0").selectOption("seal");
  await expect(page.getByTestId("imported-cabinet")).toContainText("fills a hole in the model");
  await expect(page.getByTestId("mount-hole-0")).toHaveValue("seal");

  // Free marks: wait for the place mode, then click near the middle of the view.
  await page.getByTestId("place-seal-hole").click();
  await expect(page.getByTestId("place-seal-hole")).toHaveText("Click the model…");
  await view.click({ position: { x: box.width * 0.5, y: box.height * 0.5 } });
  await expect(page.getByTestId("marked-holes")).toContainText("fills a hole in the model", { timeout: 15_000 });
  await page.getByTestId("place-opening-hole").click();
  await expect(page.getByTestId("place-opening-hole")).toHaveText("Click the model…");
  await view.click({ position: { x: box.width * 0.48, y: box.height * 0.58 } });
  await expect(page.getByTestId("marked-holes")).toContainText("moulded port mouth", { timeout: 15_000 });
});

test("a 3MF assembly in centimeters, with a component and moved items, imports as its parts", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });

  // The same shell as above in cm, placed 10 cm to the right; the wool is a component of another object.
  const model = `<?xml version="1.0" encoding="UTF-8"?>
<model unit="centimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
<resources>
<object id="1" type="model" name="shell">${meshXml([
    { lo: [0, 0, 0], hi: [16, 18, 25], out: true },
    { lo: [0.8, 0.8, 0.8], hi: [15.2, 17.2, 24.2], out: false },
  ])}</object>
<object id="2" type="model" name="wool-mesh">${meshXml([{ lo: [0, 0, 0], hi: [12, 6.5, 10], out: true }])}</object>
<object id="3" type="model" name="wool"><components><component objectid="2" transform="1 0 0 0 1 0 0 0 1 2 10 2"/></components></object>
</resources>
<build><item objectid="1" transform="1 0 0 0 1 0 0 0 1 10 0 0"/><item objectid="3" transform="1 0 0 0 1 0 0 0 1 10 0 0"/></build>
</model>`;
  const file = zip({
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>`,
    "3D/3dmodel.model": model,
  });
  await page.getByTestId("import-file").setInputFiles([{ name: "speaker.3mf", mimeType: "model/3mf", buffer: file }]);
  await expect(page.getByTestId("import-message")).toContainText("Imported 2 parts");
  await expect(page.getByTestId("imported-parts")).toContainText("shell");
  await expect(page.getByTestId("imported-parts")).toContainText("wool");

  await page.getByTestId("place-mount-0").click();
  const view = page.getByTestId("cabinet-view").locator("canvas");
  const box = (await view.boundingBox())!;
  await view.click({ position: { x: box.width * 0.45, y: box.height * 0.55 } });
  await expect(page.getByTestId("imported-cabinet")).toContainText("Sound source 1 (test-woofer-a): at", { timeout: 15_000 });
  // Both parts solid: the shell and the 0.78 L block, less one driver hole.
  await expect(page.getByRole("button", { name: "Export print file" })).toBeEnabled({ timeout: 30_000 });
  const solidM3 = Number(await page.getByTestId("solid-volume").innerText());
  const expected = 0.16 * 0.18 * 0.25 - 0.144 * 0.164 * 0.234 + 0.12 * 0.065 * 0.1;
  expect(solidM3).toBeLessThan(expected);
  expect(solidM3).toBeGreaterThan(expected - 2 * Math.PI * 0.059 * 0.059 * 0.008);
});
