import path from "node:path";

import { expect, test } from "@playwright/test";

test("the lab generates cabinets, ranks them, checks the best in 3D, and opens one", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
  const started = Date.now();
  await page.getByTestId("generate-start").click();
  await expect(page.getByTestId("generate-table")).toBeVisible({ timeout: 240_000 });
  console.log(`first generated cabinets after ${((Date.now() - started) / 1000).toFixed(0)} s`);
  await expect(page.getByTestId("generate-note")).toHaveText("The generator finished.", { timeout: 540_000 });
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
