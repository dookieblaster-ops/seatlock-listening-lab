import path from "node:path";

import { expect, test } from "@playwright/test";

test("the 3D model shapes the inside and offers the result as a design", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/sealed-test.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("formula-solid")).toBeVisible({ timeout: 20_000 });
  await page.getByTestId("shape-start").click();
  await expect(page.getByTestId("shape-result")).toBeVisible({ timeout: 200_000 });
  await expect(page.getByTestId("shape-summary")).toContainText("dB");
  await expect(page.getByTestId("shape-graph")).toBeVisible();
});
