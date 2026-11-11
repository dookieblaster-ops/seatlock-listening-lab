import { expect, test } from "@playwright/test";

test("the outside in 3D runs at both cell sizes and is drawn over the everyday model", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 15_000 });
  await page.getByTestId("outside-run").click();
  await expect(page.getByTestId("outside-note")).toContainText("finished: 10 mm in", { timeout: 240_000 });
  await expect(page.getByTestId("outside-note")).toContainText("5 mm in");
  await expect(page.getByTestId("outside-spl-graph")).toBeVisible();
  const label = page.getByTestId("outside-spl-graph-fidelity");
  await expect(label).toContainText("exterior radiation (5 mm)");
  await expect(label).toContainText("Full 3D outside the cabinet at 5 mm cells up to 6864 Hz");
  await expect(page.getByTestId("outside-uncertainty")).toContainText("The 10 mm and 5 mm grids differ by");
});
