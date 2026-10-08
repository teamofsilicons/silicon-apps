import { test, expect } from "@playwright/test";
import { account, makeApp, mock } from "./fixtures";

const scout = {
  uuid: "silicon-scout",
  id: "si:scout",
  display_name: "Scout",
  joined_at: "2026-10-08T12:00:00Z",
};
async function authors(page: Parameters<typeof mock>[0], total = 2) {
  const app = {
    ...makeApp(),
    published: true,
    authors: [...makeApp().authors, scout],
  };
  await mock(page, false, app);
  await page.route("**/v1/authors/*", async (route) => {
    const url = new URL(route.request().url());
    const who = url.pathname.endsWith(scout.uuid) ? scout : account;
    const offset = Number(url.searchParams.get("offset") || "0");
    const items = Array.from(
      { length: Math.min(24, Math.max(0, total - offset)) },
      (_, i) => ({
        ...app,
        app_id: `tool-${offset + i + 1}`,
        name: `Tool ${offset + i + 1}`,
      }),
    );
    await route.fulfill({ json: { ...who, items, total } });
  });
}

test("all co-authors appear between the title and metadata and open their own profiles", async ({
  page,
}) => {
  await authors(page);
  await page.goto("/store/test-app");
  const byline = page.getByRole("list", { name: "App authors" });
  await expect(byline.getByRole("link")).toHaveCount(2);
  await expect(
    byline.getByRole("link", { name: "Author", exact: true }),
  ).toHaveAttribute("href", "/authors/test-author");
  const title = await page
    .getByRole("heading", { name: "A useful app", exact: true })
    .boundingBox();
  const links = await byline.boundingBox();
  const metadata = await page
    .getByText("Public app", { exact: true })
    .boundingBox();
  expect(title!.y + title!.height).toBeLessThanOrEqual(links!.y);
  expect(links!.y + links!.height).toBeLessThanOrEqual(metadata!.y);
  await byline.getByRole("link", { name: "Scout", exact: true }).click();
  await expect(page).toHaveURL(/\/authors\/silicon-scout$/);
  await expect(
    page.getByRole("heading", { name: "Scout", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("si:scout", { exact: true })).toBeVisible();
  await expect(page.locator(".app-card")).toHaveCount(2);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Published apps", exact: true }),
  ).toBeVisible();
});

test("author profiles paginate without losing identity and preserve the page on reload", async ({
  page,
}) => {
  await authors(page, 25);
  await page.goto("/authors/test-author");
  await expect(page.locator(".app-card")).toHaveCount(24);
  await page.getByRole("button", { name: "Next", exact: true }).click();
  await expect(page).toHaveURL(/page=2/);
  await expect(page.locator(".app-card")).toHaveCount(1);
  await expect(
    page.getByRole("heading", { name: "Author", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Tool 25", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Previous", exact: true }).click();
  await expect(page.locator(".app-card")).toHaveCount(24);
});

test("theme follows the system, persists overrides, and stays synchronized with Settings", async ({
  page,
}) => {
  await authors(page);
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await page.goto("/store/test-app");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Switch to light mode" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.goto("/settings");
  await expect(page.getByLabel("Theme", { exact: true })).toHaveValue("light");
  await page.getByLabel("Theme", { exact: true }).selectOption("system");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Switch to light mode" }).click();
  await expect(page.getByLabel("Theme", { exact: true })).toHaveValue("light");
});

for (const width of [390, 1440]) {
  for (const theme of ["light", "dark"] as const) {
    test(`author surfaces and theme controls at ${width}px in ${theme} mode`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize({ width, height: 1000 });
      await page.emulateMedia({ colorScheme: theme });
      await authors(page);
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      for (const [name, path] of [
        ["app", "/store/test-app"],
        ["profile", "/authors/test-author"],
      ]) {
        await page.goto(path);
        await expect(page.locator("h1")).toBeVisible();
        await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
        await expect(
          page.getByRole("button", {
            name: `Switch to ${theme === "light" ? "dark" : "light"} mode`,
          }),
        ).toBeVisible();
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: testInfo.outputPath(`${name}-${theme}-${width}.png`),
          fullPage: true,
        });
      }
      expect(errors).toEqual([]);
    });
  }
}
