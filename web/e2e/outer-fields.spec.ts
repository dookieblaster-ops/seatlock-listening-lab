import path from "node:path";

import { expect, test } from "@playwright/test";

test("a sealed box still shows width height depth fields", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("outer-kind")).toContainText("box", { timeout: 20_000 });
  await expect(page.getByTestId("box-width-mm")).toBeVisible();
  await expect(page.getByTestId("tube-length-mm")).toHaveCount(0);
});

test("a PVC tube shows tube fields and editing length updates the air volume", async ({ page }) => {
  await page.goto("/");
  const example = path.resolve(
    import.meta.dirname,
    "../../designs/examples/pvc-tube.seatlock.json",
  );
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("outer-kind")).toContainText("tube", { timeout: 20_000 });
  await expect(page.getByTestId("tube-length-mm")).toBeVisible();
  await expect(page.getByTestId("tube-stock-id")).toHaveValue("charlotte-pvc-sch40-4in");
  await expect(page.getByTestId("box-width-mm")).toHaveCount(0);

  const air = page.getByTestId("formula-air");
  await expect(air).toBeVisible({ timeout: 20_000 });
  const before = Number(await air.innerText());
  await page.getByTestId("tube-length-mm").fill("600");
  await expect
    .poll(async () => Number(await air.innerText()), { timeout: 20_000 })
    .toBeGreaterThan(before * 1.1);
});

test("an egg shows revolved fields instead of box width", async ({ page }) => {
  await page.goto("/");
  const example = path.resolve(import.meta.dirname, "../../designs/examples/egg.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("outer-kind")).toContainText("revolved", { timeout: 20_000 });
  await expect(page.getByTestId("revolved-diameter-mm")).toBeVisible();
  await expect(page.getByTestId("revolved-length-mm")).toBeVisible();
  await expect(page.getByTestId("box-width-mm")).toHaveCount(0);
});
