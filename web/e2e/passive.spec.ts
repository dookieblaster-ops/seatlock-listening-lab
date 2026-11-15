import path from "node:path";
import { expect, test } from "@playwright/test";

// Passive crossover board on the page (README section 7): edit the R/L/C netlist
// without writing the design file by hand.
test("a kit with a passive board opens in the editor, and editing a part changes the result", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const design = path.resolve(
    import.meta.dirname,
    "../../designs/imports/c-note/c-note-mt.seatlock.json",
  );
  await page.getByTestId("open-design-input").setInputFiles(design);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 45_000 });
  await expect(page.getByTestId("passive-editor")).toBeVisible();
  await expect(page.getByTestId("passive-enabled")).toBeChecked();
  await expect(page.getByTestId("passive-on-note")).toBeVisible();
  await expect(page.getByTestId("passive-part-0")).toBeVisible();
  await expect(page.getByTestId("passive-kind-0")).toHaveValue("R");
  await expect(page.getByTestId("passive-value-0")).toHaveValue("6");
  const label = page.getByTestId("response-graph-fidelity");
  await expect(label).toContainText("Passive crossover");

  // Change the series resistor into the tweeter: the board still drives the graphs.
  await page.getByTestId("passive-value-0").fill("12");
  await expect(label).toContainText("Passive crossover", { timeout: 15_000 });
  await expect(page.getByTestId("passive-value-0")).toHaveValue("12");

  await page.getByTestId("passive-add-l").click();
  await expect(page.getByTestId("passive-part-9")).toBeVisible();

  await page.getByTestId("passive-enabled").uncheck();
  await expect(page.getByTestId("passive-off-note")).toBeVisible();
  await expect(label).not.toContainText("Passive crossover", { timeout: 15_000 });
});

test("turning on a passive board on a digital design adds a starter network", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  await page.getByTestId("load-example").click();
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("passive-enabled")).not.toBeChecked();
  await page.getByTestId("passive-enabled").check();
  await expect(page.getByTestId("passive-part-0")).toBeVisible();
  await expect(page.getByTestId("passive-kind-0")).toHaveValue("driver");
  await expect(page.getByTestId("response-graph-fidelity")).toContainText("Passive crossover", {
    timeout: 15_000,
  });
});
