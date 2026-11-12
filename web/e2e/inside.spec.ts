import path from "node:path";
import { expect, test } from "@playwright/test";

// The inside of the cabinet on the page (README 6.2, Level 3 track E): air spaces and air
// paths with bends and tapers, edited without writing the design file by hand.
test("a folded line opens in the inside editor, and editing its path changes the result", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/folded-line.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("inside-editor")).toBeVisible();
  await expect(page.getByTestId("chamber-0")).toBeVisible();
  await expect(page.getByTestId("duct-shape-0")).toHaveValue("rect");
  const pathField = page.getByTestId("duct-path-0");
  await expect(pathField).toHaveValue("80, 190, 600\n80, 190, 60\n80, 60, 60\n80, 60, 350\n80, 0, 350");
  const label = page.getByTestId("response-graph-fidelity");
  await expect(label).toContainText("3 bends in the ducts");

  // One fold fewer: the line now leaves the front near the bottom.
  await pathField.fill("80, 190, 600\n80, 190, 60\n80, 60, 60\n80, 0, 60");
  await expect(label).toContainText("2 bends in the ducts", { timeout: 15_000 });

  // A half-typed line does not reach the design.
  await pathField.fill("80, 190, 600\n80, 190");
  await expect(page.getByText("Each line needs three numbers")).toBeVisible();
  await expect(label).toContainText("2 bends in the ducts");

  await page.getByTestId("add-duct").click();
  await expect(page.getByTestId("duct-1")).toBeVisible();
});
