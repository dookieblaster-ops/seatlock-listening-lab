import { expect, test } from "@playwright/test";

test("the boundary-element outside runs and is drawn over the everyday model", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 15_000 });
  await page.getByTestId("bem-run").click();
  await expect(page.getByTestId("bem-note")).toContainText("finished in", { timeout: 120_000 });
  await expect(page.getByTestId("bem-spl-graph")).toBeVisible();
  const label = page.getByTestId("bem-spl-graph-fidelity");
  await expect(label).toContainText("bem radiation");
  await expect(label).toContainText("outer shape");
  await expect(page.getByTestId("bem-timing")).toContainText("Boundary-element mesh");
});
