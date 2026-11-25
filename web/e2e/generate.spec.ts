import path from "node:path";

import { expect, test } from "@playwright/test";

// One attempt only: a hung or over-long generate already burns a long Check; retrying doubles CI.
test.describe.configure({ retries: 0 });

test("the lab generates cabinets, ranks them, checks the best in 3D, and opens one", async ({ page }) => {
  test.setTimeout(1_200_000);
  // Fewer candidates per family so Actions Wasm finishes within the Check budget.
  await page.addInitScript(() => {
    (window as unknown as { __SEATLOCK_GENERATE_EVALS?: number }).__SEATLOCK_GENERATE_EVALS = 6;
  });
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
  const started = Date.now();
  await page.getByTestId("generate-start").click();
  await expect(page.getByTestId("generate-table")).toBeVisible({ timeout: 300_000 });
  console.log(`first generated cabinets after ${((Date.now() - started) / 1000).toFixed(0)} s`);
  try {
    await expect(page.getByTestId("generate-note")).toHaveText("The generator finished.", {
      timeout: 1_050_000,
    });
  } catch (error) {
    const progress = await page.getByTestId("generate-progress").textContent().catch(() => "(no progress)");
    const note = await page.getByTestId("generate-note").textContent();
    console.log(`generate stuck. note=${note}; progress=${progress}`);
    throw error;
  }
  console.log(`generator finished after ${((Date.now() - started) / 1000).toFixed(0)} s`);
  await expect(page.getByTestId("generate-verified")).toContainText("3D check of the top cabinet");
  const rows = page.getByTestId("generate-table").locator("tbody tr");
  expect(await rows.count()).toBeGreaterThan(5);
  await expect(rows.first()).toContainText("Dayton Audio");
  // Opening the top cabinet makes it the design: a real driver, and a print file on offer.
  await rows.first().getByRole("button", { name: "Open" }).click();
  await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole("button", { name: "Export print file" })).toBeEnabled({ timeout: 20_000 });
});
