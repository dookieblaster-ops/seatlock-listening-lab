import { expect, test } from "@playwright/test";

test("the room estimate adds an in-room curve and changes the score", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("in-room-graph")).toHaveCount(0);
  await page.getByTestId("room-toggle").check();
  const graph = page.getByTestId("in-room-graph");
  await expect(graph).toBeVisible({ timeout: 15_000 });
  const label = page.getByTestId("in-room-graph-fidelity");
  await expect(label).toContainText("simple room");
  await expect(label).toContainText("The room is an estimate");
  await expect(page.getByTestId("room-wall-m")).toHaveValue("0.5");
});
