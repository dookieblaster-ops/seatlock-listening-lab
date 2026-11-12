import path from "node:path";
import { expect, test } from "@playwright/test";

// The `rear-panel` half of CHK-E2E-3 in the browser (README sections 5.9 and 12): a panel
// speaker on the back of a cabinet that is not a box, with the room, opens and shows results.
test("a rear-facing panel speaker opens, is shown as a panel, and is heard by way of the room", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("engine-version")).toContainText("0.1.0", { timeout: 15_000 });
  const example = path.resolve(import.meta.dirname, "../../designs/examples/rear-panel.seatlock.json");
  await page.getByTestId("open-design-input").setInputFiles(example);
  await expect(page.getByTestId("response-graph")).toBeVisible({ timeout: 30_000 });

  // The panel is a panel, not a driver with an empty name.
  const note = page.getByTestId("panel-note-1");
  await expect(note).toContainText("panel speaker");
  await expect(note).toContainText("300 x 200 mm");
  await expect(page.getByTestId("driver-id-0")).toHaveValue("dayton-tcp115-4");
  await expect(page.getByTestId("driver-id-1")).toHaveCount(0);
  await expect(page.getByTestId("driver-note")).toHaveCount(1);

  // The room is on in this design, and the label says what the panel model covers.
  const graph = page.getByTestId("in-room-graph");
  await expect(graph).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("in-room-graph-fidelity")).toContainText("resonances up to");
  await expect(page.getByTestId("scores")).toBeVisible();
});
