import { expect, test } from "@playwright/test";

test("another driver can be added on the page", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("driver-id-0")).toBeVisible();
  await page.getByTestId("add-driver").click();
  await expect(page.getByTestId("driver-id-1")).toBeVisible();
  await expect(page.getByTestId("extra-driver-1")).toBeVisible();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });
});
