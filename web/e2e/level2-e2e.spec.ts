import { readFileSync } from "node:fs";
import path from "node:path";

import { expect, test } from "@playwright/test";

const expected = JSON.parse(readFileSync(new URL("./chk-e2e-2.json", import.meta.url), "utf8")) as {
  bass_hz: string;
  evenness_db: string;
  max_clean_db: string;
  cost: string;
};

async function download(page: import("@playwright/test").Page, testId: string) {
  const [file] = await Promise.all([page.waitForEvent("download"), page.getByTestId(testId).click()]);
  const chunks: Buffer[] = [];
  for await (const chunk of await file.createReadStream()) {
    chunks.push(Buffer.from(chunk));
  }
  return { name: file.suggestedFilename(), body: Buffer.concat(chunks) };
}

test("CHK-E2E-2 two-way-vented simulates, corrects, and exports", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/two-way-vented.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("scores")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("export-stl")).toBeVisible({ timeout: 60_000 });

  await page.getByRole("button", { name: "Apply correction" }).click();
  await expect(page.getByTestId("correction-note")).not.toBeEmpty({ timeout: 20_000 });
  const scores = page.getByTestId("scores");
  console.log("CHK_E2E_2_SCORES", await scores.textContent());
  await expect(scores).toContainText(`Bass limit ${expected.bass_hz} Hz`);
  await expect(scores).toContainText(`Evenness ${expected.evenness_db} dB`);
  await expect(scores).toContainText(`Maximum clean level ${expected.max_clean_db} dB`);
  await expect(scores).toContainText(expected.cost);

  const stl = await download(page, "export-stl");
  expect(stl.name).toContain(".stl");
  expect(stl.body.length).toBeGreaterThan(84);
  const filters = await download(page, "export-filters");
  expect(filters.name).toContain("filters.txt");
  expect(filters.body.toString("utf8")).toContain("biquad");
  await expect(page.getByTestId("cone-limit")).toBeVisible();
});
