import { expect, test } from "@playwright/test";

test("reference build, build methods, and the optimizer show cost beside score", async ({ page }) => {
  // Preference scoring runs CTA-2034 early reflections on every candidate (README 11.1).
  test.setTimeout(1_500_000);
  await page.goto("/");
  await page.getByTestId("load-example").click();
  await expect(page.getByRole("button", { name: "Compare with a reference build" })).toBeEnabled({
    timeout: 20_000,
  });

  await page.getByRole("button", { name: "Compare with a reference build" }).click();
  const reference = page.getByTestId("reference-table");
  await expect(reference).toBeVisible({ timeout: 180_000 });
  await expect(reference.locator("tbody tr")).toHaveCount(2);
  await expect(reference).toContainText("Reference build");
  await expect(reference).toContainText("$");
  await expect(reference).toContainText("Preference");

  await page.getByRole("button", { name: "Compare build methods" }).click();
  const methods = page.getByTestId("methods-table");
  await expect(methods).toBeVisible({ timeout: 300_000 });
  await expect(methods).toContainText("Printed box (PLA)");
  await expect(methods).toContainText("MDF");

  const started = Date.now();
  await page.getByRole("button", { name: "Find the cheapest design" }).click();
  const ranked = page.getByTestId("optimizer-table");
  await expect(ranked.locator("tbody tr").first()).toBeVisible({ timeout: 600_000 });
  console.log("OPTIMIZER_FIRST_RESULT_S", ((Date.now() - started) / 1000).toFixed(1));
  await expect(page.getByTestId("cost-plot")).toBeVisible();
  await expect(page.getByTestId("cost-quality-note")).toHaveText("The optimizer finished.", {
    timeout: 1_200_000,
  });
  console.log("OPTIMIZER_DONE_S", ((Date.now() - started) / 1000).toFixed(1));
  await expect(page.getByTestId("optimizer-verified")).toContainText("Full 3D check of the top design");
  console.log("OPTIMIZER_VERIFIED", await page.getByTestId("optimizer-verified").textContent());

  await ranked.getByRole("button", { name: "Open" }).first().click();
  await expect(page.getByTestId("scores")).toBeVisible();
});
