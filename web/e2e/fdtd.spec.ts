import { expect, test } from "@playwright/test";

test("full 3D run draws a curve and can be canceled", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("load-example").click();
  await expect(page.getByRole("button", { name: "Run full 3D" })).toBeVisible({ timeout: 20_000 });

  await page.getByRole("button", { name: "Run full 3D" }).click();
  await page.getByRole("button", { name: "Cancel full 3D" }).click();
  await expect(page.getByTestId("fdtd-note")).toHaveText("Full 3D canceled.");
  await expect(page.getByTestId("fdtd-error")).toHaveCount(0);

  await page.getByRole("button", { name: "Run full 3D" }).click();
  await expect(page.getByTestId("impedance-3d-graph")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("fdtd-error")).toHaveCount(0);
  await expect(page.getByTestId("fdtd-note")).not.toContainText("failed");
});
