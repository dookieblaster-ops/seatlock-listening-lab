import path from "node:path";
import { expect, test } from "@playwright/test";

// Novel materials on the page (README sections 5.9, 6.2, 9.1, 14): a layered panel speaker,
// and a printed cabinet with filled or two-filament walls.
test("a sandwich panel and a sand-filled wall are chosen on the page, simulated, and priced", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/rear-panel.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });

  // The panel becomes PLA skins on a honeycomb core.
  await page.getByTestId("panel-kind-1").selectOption("layers");
  await page.getByTestId("panel-sandwich-1").click();
  await expect(page.getByTestId("panel-layers-1")).toBeVisible();
  await expect(page.getByTestId("panel-layer-material-1-1")).toHaveValue("thermhex-pp-honeycomb");
  await expect(page.getByTestId("response-graph-fidelity")).toContainText("The panel is 3 layers", { timeout: 30_000 });
  await expect(page.getByRole("cell", { name: /^Panel, Printed PLA/ }).first()).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("cell", { name: /^Panel, ThermHex/ })).toBeVisible();

  // The cabinet walls: two skins with play sand between them, then a foaming core.
  await expect(page.getByTestId("build-material")).toHaveValue("pla");
  await page.getByTestId("wall-kind").selectOption("filled");
  await expect(page.getByTestId("wall-fill")).toHaveValue("quikrete-play-sand");
  await expect(page.getByRole("cell", { name: /^Wall fill, Quikrete Premium Play Sand/ })).toBeVisible({ timeout: 30_000 });
  await page.getByTestId("wall-kind").selectOption("two_material");
  await page.getByTestId("wall-core").selectOption("lw_pla");
  await expect(page.getByRole("cell", { name: /^Printed wall core, colorFabb LW-PLA/ })).toBeVisible({ timeout: 30_000 });
});
