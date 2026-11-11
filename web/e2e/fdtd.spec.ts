import path from "node:path";
import { expect, test } from "@playwright/test";

test("full 3D run on a sealed box: both cell sizes, uncertainty, and cancel", async ({
  page,
}) => {
  test.setTimeout(180_000);
  await page.goto("/");
  await page.getByTestId("load-example").click();
  await expect(page.getByRole("button", { name: "Run full 3D" })).toBeEnabled({
    timeout: 20_000,
  });

  await page.getByRole("button", { name: "Run full 3D" }).click();
  await page.getByRole("button", { name: "Cancel full 3D" }).click();
  await expect(page.getByTestId("fdtd-note")).toHaveText("Full 3D canceled.");
  await expect(page.getByTestId("fdtd-error")).toHaveCount(0);

  await page.getByRole("button", { name: "Run full 3D" }).click();
  await expect(page.getByTestId("fdtd-note")).toContainText(
    "Full 3D finished: 10 mm in",
    { timeout: 120_000 },
  );
  await expect(page.getByTestId("fdtd-note")).toContainText("5 mm in");
  await expect(page.getByTestId("impedance-3d-graph")).toBeVisible();
  await expect(page.getByTestId("fdtd-spl-graph")).toBeVisible();
  await expect(page.getByTestId("fdtd-uncertainty")).toContainText(
    "dB below 300 Hz",
  );
  await expect(page.getByTestId("fdtd-error")).toHaveCount(0);
  console.log(
    "SEALED_FULL_3D",
    await page.getByTestId("fdtd-note").textContent(),
  );
  console.log(
    "SEALED_UNCERTAINTY",
    await page.getByTestId("fdtd-uncertainty").textContent(),
  );
});

for (const name of ["hex-prism", "two-way-vented"]) {
  test(`${name}: full 3D runs on the cabinet as the builder makes it`, async ({
    page,
  }) => {
    test.setTimeout(240_000);
    await page.goto("/");
    const example = path.resolve(
      import.meta.dirname,
      `../../designs/examples/${name}.seatlock.json`,
    );
    await page.getByTestId("open-design-input").setInputFiles(example);
    await expect(page.getByRole("button", { name: "Run full 3D" })).toBeEnabled(
      { timeout: 20_000 },
    );
    await page.getByRole("button", { name: "Run full 3D" }).click();
    await expect(page.getByTestId("fdtd-spl-graph")).toBeVisible({
      timeout: 180_000,
    });
    await expect(
      page.getByTestId("fdtd-spl-graph").locator(".."),
    ).toContainText(/Full 3D at (10|5) mm cells/);
    await expect(page.getByTestId("fdtd-error")).toHaveCount(0);
    console.log(
      "FULL_3D",
      name,
      await page.getByTestId("fdtd-note").textContent(),
    );
    await page.getByRole("button", { name: "Cancel full 3D" }).click();
  });
}
