import { expect, test } from "@playwright/test";

test("changing the box size changes the response", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  const level = page.getByTestId("spl-40hz");
  await expect(level).toBeVisible({ timeout: 15_000 });
  const before = await level.innerText();
  await page.getByTestId("box-width-mm").fill("400");
  await expect(level).not.toHaveText(before, { timeout: 15_000 });
});
