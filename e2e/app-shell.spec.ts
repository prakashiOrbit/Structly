import { test, expect } from "@playwright/test";

test("shows the app shell and the connections pane by default", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Structly")).toBeVisible();
  await expect(page.getByText("Select a connection to get started.")).toBeVisible();
});
