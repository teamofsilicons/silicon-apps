import { test, expect, type Route } from "@playwright/test";
import { account, makeApp, mock } from "./fixtures";
import type { App, Invite, Package, Release, Review } from "../src/types";

const reply = (route: Route, body: unknown, status = 200) =>
  route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
const secondAuthor = {
  uuid: "second-immutable-uuid",
  id: "si:helper",
  display_name: "Helper",
  joined_at: "2026-10-09T12:00:00Z",
};
const release: Release = {
  id: "dev-release",
  app_id: "test-app",
  channel: "development",
  version: "0.3.0",
  package_ids: ["package-one"],
  notes: "A tested release",
  created_at: "2026-10-08T12:00:00Z",
};
const pkg: Package = {
  id: "package-one",
  target: "macos-aarch64",
  sha256: "a".repeat(64),
  size: 1024,
  command: "useful",
  created_at: "2026-10-08T12:00:00Z",
  validation: [
    {
      command: "--help",
      exit_code: 0,
      stdout: "Useful help",
      stderr: "Useful diagnostic",
      passed: true,
      expected: "Exit 0 with help",
    },
    {
      command: "accounts --json",
      exit_code: 0,
      stdout: '{"app_id":"test-app"}',
      stderr: "",
      passed: true,
      expected: "Matching app_id",
    },
    {
      command: "login status --json",
      exit_code: 0,
      stdout: '{"authenticated":false}',
      stderr: "",
      passed: true,
      expected: "Signed out",
    },
  ],
};
const pixel = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j1ioAAAAASUVORK5CYII=",
  "base64",
);

test("review CRUD keeps one review per immutable account and updates visible totals", async ({
  page,
}) => {
  await mock(page, true, { published: true });
  let review: Review | null = null;
  await page.route("**/v1/apps/test-app/review", (route) => {
    review =
      route.request().method() === "DELETE"
        ? null
        : {
            ...route.request().postDataJSON(),
            uuid: account.uuid,
            id: account.id,
            updated_at: "2026-10-08T12:00:00Z",
          };
    return reply(route, review || {});
  });
  await page.route("**/v1/apps/test-app/reviews", (route) =>
    reply(route, {
      items: review ? [review] : [],
      rating: review?.rating || null,
      count: review ? 1 : 0,
    }),
  );
  await page.route("**/v1/apps/test-app", (route) =>
    reply(route, {
      ...makeApp(),
      published: true,
      rating: review?.rating || null,
      review_count: review ? 1 : 0,
    }),
  );
  await page.goto("/store/test-app");
  await page.getByRole("button", { name: "Write a review" }).click();
  await page.getByLabel("Review (optional)").fill("Useful first review");
  await expect(page.getByLabel("Review (optional)")).toHaveAttribute(
    "maxlength",
    "600",
  );
  await page.getByRole("button", { name: "Save review" }).click();
  await expect(page.locator(".review-list .review")).toHaveCount(1);
  await expect(page.locator(".detail-stats")).toContainText("1 reviews");
  await page.getByRole("button", { name: "Edit your review" }).click();
  await page.getByLabel("2 stars", { exact: true }).check();
  await page.getByLabel("Review (optional)").fill("");
  await page.getByRole("button", { name: "Save review" }).click();
  await expect(page.locator(".review-list .review")).toHaveCount(1);
  await expect(page.locator(".review-list .stars")).toHaveAttribute(
    "aria-label",
    "2 out of 5 stars",
  );
  await page.getByRole("button", { name: "Edit your review" }).click();
  await page.getByRole("button", { name: "Remove review" }).click();
  await expect(page.getByText("Your review has been removed.")).toBeVisible();
  await expect(page.locator(".review-list .review")).toHaveCount(0);
  await expect(page.locator(".detail-stats")).toContainText("0 reviews");
});

test("store preserves relevance order, private filter and development exact install syntax", async ({
  page,
}) => {
  await mock(page, true, {
    published: true,
    latest_development: release,
    targets: ["macos-aarch64"],
  });
  const requests: URL[] = [];
  await page.route("**/v1/apps?**", (route) => {
    requests.push(new URL(route.request().url()));
    return reply(route, {
      items: [
        { ...makeApp(), name: "Exact match", rating: 1 },
        { ...makeApp(), app_id: "weaker", name: "Weaker result", rating: 5 },
      ],
      total: 2,
    });
  });
  await page.goto("/store");
  await page.getByRole("searchbox", { name: "Search apps" }).fill("exat");
  await expect
    .poll(() => requests.some((x) => x.searchParams.get("q") === "exat"))
    .toBe(true);
  await expect(page.locator(".app-card h3").first()).toHaveText("Exact match");
  await page.getByRole("button", { name: "Private", exact: true }).click();
  await expect
    .poll(() =>
      requests.some((x) => x.searchParams.get("visibility") === "private"),
    )
    .toBe(true);
  await page.goto("/store/test-app");
  await expect(page.locator(".detail-stats")).toContainText(
    "v0.3.0 · Development",
  );
  await page.getByRole("button", { name: "Install app" }).click();
  await expect(
    page.getByText(/No production release is available yet/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Development", exact: true }).click();
  await page.getByLabel("Exact version (optional)").fill("0.3");
  await expect(
    page.getByText("Use an x.y.z version, such as 1.2.3."),
  ).toBeVisible();
  await page.getByLabel("Exact version (optional)").fill("0.3.0");
  await expect(page.getByRole("dialog").locator("code")).toHaveText(
    "apps install 'test-app>dev@0.3.0'",
  );
});

test("store documentation links use the combined developer docs", async ({
  page,
}) => {
  await mock(page);
  await page.goto("/store");
  for (const name of ["Docs", "Documentation"])
    await expect(page.getByRole("link", { name, exact: true })).toHaveAttribute(
      "href",
      "https://developers.teamofsilicons.com/docs/apps",
    );
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Open navigation" }).click();
  await expect(
    page
      .getByRole("navigation", { name: "Mobile navigation" })
      .getByRole("link", { name: "Docs", exact: true }),
  ).toHaveAttribute("href", "https://developers.teamofsilicons.com/docs/apps");
});
