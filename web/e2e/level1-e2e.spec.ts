import { readFileSync } from "node:fs";

import { expect, test } from "@playwright/test";

const expected = JSON.parse(readFileSync(new URL("./chk-e2e-1.json", import.meta.url), "utf8")) as {
  bass_hz: string;
  evenness_db: string;
  max_clean_db: string;
  cost: string;
};

test("CHK-E2E-1 sealed-test simulates, corrects, and exports", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("scores")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("export-stl")).toBeVisible({ timeout: 20_000 });

  await page.getByRole("button", { name: "Apply correction" }).click();
  await expect(page.getByTestId("correction-note")).not.toBeEmpty({ timeout: 20_000 });
  const scores = page.getByTestId("scores");
  await expect(scores).toContainText(`Bass limit ${expected.bass_hz} Hz`);
  await expect(scores).toContainText(`Evenness ${expected.evenness_db} dB`);
  await expect(scores).toContainText(`Maximum clean level ${expected.max_clean_db} dB`);
  await expect(scores).toContainText(expected.cost);

  const [stl] = await Promise.all([
    page.waitForEvent("download"),
    page.getByTestId("export-stl").click(),
  ]);
  expect(stl.suggestedFilename()).toContain(".stl");
  const stlBody = await stl.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stlBody) {
    chunks.push(Buffer.from(chunk));
  }
  expect(Buffer.concat(chunks).length).toBeGreaterThan(84);

  const [filters] = await Promise.all([
    page.waitForEvent("download"),
    page.getByTestId("export-filters").click(),
  ]);
  expect(filters.suggestedFilename()).toContain("filters.txt");
  const textStream = await filters.createReadStream();
  const textChunks: Buffer[] = [];
  for await (const chunk of textStream) {
    textChunks.push(Buffer.from(chunk));
  }
  const filterText = Buffer.concat(textChunks).toString("utf8");
  expect(filterText).toContain("biquad");
  await expect(page.getByTestId("filter-text")).toContainText("biquad");
  await expect(page.getByTestId("cone-limit")).toBeVisible();
  await expect(page.getByTestId("listen-note")).toContainText("headphones or speakers");
});
