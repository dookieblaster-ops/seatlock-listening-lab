import { expect, test } from "@playwright/test";

test("loads the engine and round-trips sealed-test", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });

  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("design-name")).toHaveText("sealed-test.seatlock.json");
  await expect(page.getByTestId("design-status")).toContainText("no warnings", { timeout: 10_000 });
});
