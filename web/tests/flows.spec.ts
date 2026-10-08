import { test, expect } from "@playwright/test";
import { account, mock } from "./fixtures";
test("anonymous private apps explain sign-in and never fetch private listings", async ({
  page,
}) => {
  await mock(page);
  const privateRequests: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("visibility=private")) privateRequests.push(r.url());
  });
  await page.goto("/store");
  await expect(
    page.getByRole("heading", { name: "Find your next possibility." }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Private", exact: true }).click();
  await expect(page.getByText("Log in to see private apps")).toBeVisible();
  expect(privateRequests).toHaveLength(0);
  await expect(
    page.getByRole("button", { name: "Sign in with Silicon Accounts" }),
  ).toBeVisible();
});
test("review creates one account review and confirms the result", async ({
  page,
}) => {
  const changes = await mock(page, true);
  await page.goto("/store/test-app");
  await page.getByRole("button", { name: "Write a review" }).click();
  await page.getByLabel("Review (optional)").fill("Useful");
  await page.getByRole("button", { name: "Save review" }).click();
  await expect(page.getByText("Your review has been saved.")).toBeVisible();
  expect(
    changes.find((change) => change.path.endsWith("/review"))?.body,
  ).toMatchObject({ rating: 5, text: "Useful" });
});
test("telemetry opt-out persists and suppresses following page events", async ({
  page,
}) => {
  const changes = await mock(page, true);
  await page.goto("/settings");
  await page.getByRole("switch", { name: "Share usage telemetry" }).click();
  await expect(
    page.getByRole("switch", { name: "Share usage telemetry" }),
  ).not.toBeChecked();
  const count = changes.filter((c) => c.path === "/telemetry").length;
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Explore apps" }),
  ).toBeVisible();
  expect(changes.filter((c) => c.path === "/telemetry")).toHaveLength(count);
  expect(
    await page.evaluate(() => localStorage.getItem("apps.telemetry")),
  ).toBe("false");
});
for (const width of [390, 768, 1024, 1440])
  test(`store remains within ${width}px viewport`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await mock(page, true);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    for (const path of ["/store", "/store/test-app", "/docs"]) {
      await page.goto(path);
      await expect(page.locator("h1")).toBeVisible();
      await expect
        .poll(() =>
          page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth,
          ),
        )
        .toBe(true);
    }
    expect(errors).toEqual([]);
  });
