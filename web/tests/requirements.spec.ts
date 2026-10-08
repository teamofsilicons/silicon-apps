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

test("creation accepts optional description/logo and explains availability outages", async ({
  page,
}) => {
  const changes = await mock(page, true);
  let failed = true;
  await page.route("**/v1/apps/availability/test-app", (route) =>
    failed
      ? reply(
          route,
          {
            error: {
              code: "accounts_unavailable",
              message: "Availability could not be checked.",
            },
          },
          503,
        )
      : route.fallback(),
  );
  await page.goto("/developer");
  await page.getByRole("button", { name: "Create app", exact: true }).click();
  await page.getByLabel("App name", { exact: true }).fill("A useful app");
  await page.getByLabel("App ID", { exact: true }).fill("test-app");
  await page
    .getByLabel("Description (optional)")
    .fill("An optional early introduction.");
  await page
    .getByLabel("Logo URL (optional)")
    .fill("https://example.com/logo.png");
  await expect(
    page.getByText("Availability could not be checked."),
  ).toBeVisible();
  await expect(
    page
      .getByRole("dialog")
      .getByRole("button", { name: "Create app", exact: true }),
  ).toBeDisabled();
  failed = false;
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByText("This app ID is available")).toBeVisible();
  expect(changes.filter((x) => x.path === "/apps")).toHaveLength(0);
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Create app", exact: true })
    .click();
  await expect(page.getByText("test-secret-shown-once")).toBeVisible();
  expect(changes.find((x) => x.path === "/apps")?.body).toMatchObject({
    description: "An optional early introduction.",
    logo: "https://example.com/logo.png",
  });
});

test("all seven steps are freely traversable and access edits save before blur and resume", async ({
  page,
}) => {
  const changes = await mock(page, true, { description: "", tags: [] });
  await page.goto("/developer/apps/test-app");
  for (const [step, title] of [
    [7, "Ready for the ecosystem?"],
    [4, "Connect the rest of your app"],
    [2, "Choose who can find your app"],
  ] as const) {
    await page
      .getByRole("navigation", { name: "Publishing steps" })
      .getByRole("button")
      .nth(step - 1)
      .click();
    await expect(page.getByRole("heading", { name: title })).toBeVisible();
  }
  await page.getByRole("radio", { name: /Private/ }).check();
  await page.getByLabel("Share with accounts").fill("c:alice\nsi:assistant");
  await expect
    .poll(() =>
      changes.some(
        (x) =>
          x.path.endsWith("/access") &&
          JSON.stringify(x.body.account_ids) === '["c:alice","si:assistant"]',
      ),
    )
    .toBe(true);
  await expect(page.getByLabel("Share with accounts")).toBeFocused();
  await page.getByLabel("Allowed email domains").fill("@teamofsilicons.com");
  await expect
    .poll(() =>
      changes.some(
        (x) => JSON.stringify(x.body.domains) === '["teamofsilicons.com"]',
      ),
    )
    .toBe(true);
  for (const step of [5, 3, 1, 6])
    await page
      .getByRole("navigation", { name: "Publishing steps" })
      .getByRole("button")
      .nth(step - 1)
      .click();
  await page.getByRole("button", { name: "Authors", exact: true }).click();
  await page.getByRole("button", { name: "Resume app setup" }).click();
  await expect(
    page.getByRole("heading", { name: "Stay in sync with Silicon Accounts" }),
  ).toBeVisible();
  await page.goto("/developer/apps/test-app");
  await expect(
    page.getByRole("heading", { name: "Stay in sync with Silicon Accounts" }),
  ).toBeVisible();
  expect(
    new Set(changes.map((x) => x.body.setup_step).filter(Boolean)),
  ).toEqual(new Set([1, 2, 3, 4, 5, 6, 7]));
});

test("non-admin authors can invite and rotate but cannot alter access or remove peers", async ({
  page,
}) => {
  const changes = await mock(page, true, {
    is_admin: false,
    authors: [...makeApp().authors, secondAuthor],
    visibility: "private",
    domains: ["teamofsilicons.com"],
    account_ids: ["c:alice"],
  });
  await page.goto("/developer/apps/test-app?step=2");
  await expect(page.getByRole("radio", { name: /Public/ })).toBeDisabled();
  await expect(page.getByLabel("Share with accounts")).toBeDisabled();
  await expect(page.getByLabel("Allowed email domains")).toBeDisabled();
  await page.getByRole("button", { name: "Authors", exact: true }).click();
  await expect(page.getByRole("button", { name: "Make admin" })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Remove si:helper" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Rotate app secret" }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Leave app", exact: true }),
  ).toBeEnabled();
  await page.getByLabel("Invite an author").fill("person@example.com");
  await page.getByRole("button", { name: "Send invitation" }).click();
  await expect(
    page.getByText("Invitation created for person@example.com."),
  ).toBeVisible();
  expect(changes.find((x) => x.path.endsWith("/invites"))?.body).toEqual({
    to: "person@example.com",
  });
  expect(changes.filter((x) => x.path.endsWith("/access"))).toHaveLength(0);
});

test("administrator transfer uses immutable UUID; last author cannot leave", async ({
  page,
}) => {
  const changes = await mock(page, true, {
    authors: [...makeApp().authors, secondAuthor],
  });
  await page.goto("/developer/apps/test-app");
  await page.getByRole("button", { name: "Authors", exact: true }).click();
  await page.getByRole("button", { name: "Make admin" }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "You will remain an author",
  );
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  expect(changes.find((x) => x.path.endsWith("/admin"))?.body).toEqual({
    uuid: secondAuthor.uuid,
  });
  await page.route("**/v1/apps/test-app/authors", (route) =>
    reply(route, { items: makeApp().authors }),
  );
  await page.reload();
  await page.getByRole("button", { name: "Authors", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Leave app", exact: true }),
  ).toBeDisabled();
  await expect(page.getByText(/You are the last author/)).toBeVisible();
});

test("pending invitations can be cancelled and recipients accept or decline before joining", async ({
  page,
}) => {
  await mock(page, true);
  let outgoing = [
    {
      id: "outgoing",
      app_id: "test-app",
      to: "c:friend",
      status: "pending",
      created_at: "2026-10-08T12:00:00Z",
    },
  ];
  await page.route("**/v1/apps/test-app/invites**", (route) => {
    if (route.request().method() === "DELETE") outgoing = [];
    return reply(route, { items: outgoing });
  });
  await page.goto("/developer/apps/test-app");
  await page.getByRole("button", { name: "Authors", exact: true }).click();
  await expect(page.locator(".members-list").first()).not.toContainText(
    "c:friend",
  );
  await page.getByRole("button", { name: "Cancel invitation" }).click();
  await expect(page.getByText("Invitation cancelled.")).toBeVisible();
  let incoming: Invite[] = ["accept-me", "decline-me"].map((id) => ({
    id,
    app_id: id,
    to: account.id,
    status: "pending",
    created_at: "2026-10-08T12:00:00Z",
  }));
  const actions: string[] = [];
  await page.route("**/v1/invites**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (route.request().method() === "POST") {
      actions.push(path);
      incoming = incoming.filter((x) => !path.includes(x.id));
    }
    return reply(route, { items: incoming });
  });
  await page.goto("/developer/invitations");
  await page
    .getByRole("article")
    .filter({ hasText: "accept-me" })
    .getByRole("button", { name: "Accept invitation" })
    .click();
  await expect(
    page.getByText("You are now an author of accept-me. Find it in Your apps."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Decline", exact: true }).click();
  await expect(page.getByText("Invitation declined.")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "You’re all caught up" }),
  ).toBeVisible();
  expect(actions).toEqual([
    "/v1/invites/accept-me/accept",
    "/v1/invites/decline-me/decline",
  ]);
});

test("media uploads and alt text persist, custom logos render, and alt text is not a caption", async ({
  page,
}) => {
  const changes = await mock(page, true);
  await page.route("**/v1/apps/test-app/media", (route) =>
    reply(route, { url: "/v1/apps/test-app/media/logo", kind: "image" }),
  );
  await page.route("**/v1/apps/test-app/media/*", (route) =>
    route.fulfill({ contentType: "image/png", body: pixel }),
  );
  await page.goto("/developer/apps/test-app?step=5");
  await page
    .locator('input[type="file"]')
    .first()
    .setInputFiles({ name: "logo.png", mimeType: "image/png", buffer: pixel });
  await expect(page.getByLabel("Logo URL", { exact: true })).toHaveValue(
    "/v1/apps/test-app/media/logo",
  );
  await page
    .getByLabel("Logo alt text")
    .fill("A machine-readable logo description");
  await page
    .getByLabel("Banner URL", { exact: true })
    .fill("/v1/apps/test-app/media/banner");
  await page
    .getByLabel("Banner alt text")
    .fill("A machine-readable banner description");
  await page.getByRole("button", { name: "Add URL", exact: true }).click();
  await page.getByLabel("Media URL").fill("/v1/apps/test-app/media/carousel");
  await page
    .getByLabel("Alt text for Silicons")
    .fill("A machine-readable carousel description");
  await expect(page.getByLabel("Alt text for Silicons")).toHaveAttribute(
    "maxlength",
    "10000",
  );
  await page.getByRole("button", { name: "4 Links" }).click();
  await page.getByRole("button", { name: "Add link", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("Community");
  await page
    .getByLabel("URL", { exact: true })
    .fill("https://example.com/community");
  await page
    .getByLabel("Logo URL", { exact: true })
    .fill("https://example.com/community.png");
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await page.goto("/store/test-app");
  await expect(
    page.getByRole("img", {
      name: "A machine-readable logo description",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("img", {
      name: "A machine-readable banner description",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("img", {
      name: "A machine-readable carousel description",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("A machine-readable carousel description", { exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: "Community", exact: true }).locator("img"),
  ).toHaveAttribute("src", "https://example.com/community.png");
  expect(
    changes.some(
      (x) => x.body.logo_alt === "A machine-readable logo description",
    ),
  ).toBe(true);
});

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

test("validated packages retain both output streams and releases promote with independent versions", async ({
  page,
}) => {
  await mock(page, true);
  const packages = [
    pkg,
    { ...pkg, id: "package-newer", sha256: "b".repeat(64) },
  ];
  const releases: Release[] = [];
  await page.route("**/v1/apps/test-app/packages", (route) =>
    reply(route, { items: packages }),
  );
  await page.route("**/v1/apps/test-app/releases", (route) => {
    if (route.request().method() === "POST") {
      const saved = { ...release, ...route.request().postDataJSON() };
      releases.push(saved);
      return reply(route, saved);
    }
    return reply(route, { items: releases });
  });
  await page.route(
    "**/v1/apps/test-app/releases/dev-release/promote",
    (route) => {
      const production = {
        ...release,
        id: "production-release",
        channel: "production" as const,
        version: route.request().postDataJSON().version,
        promoted_from: release.id,
      };
      releases.push(production);
      return reply(route, production);
    },
  );
  await page.goto("/developer/apps/test-app?step=3");
  await page.locator(".package-detail").first().locator("summary").click();
  await expect(
    page
      .locator(".package-detail")
      .first()
      .getByText("Useful help", { exact: true }),
  ).toBeVisible();
  await expect(
    page
      .locator(".package-detail")
      .first()
      .getByText("Useful diagnostic", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Create release", exact: true })
    .click();
  await page.getByLabel("Development version").fill("0.3.0");
  const picker = page.getByRole("dialog").locator('input[type="checkbox"]');
  await picker.nth(0).check();
  await picker.nth(1).check();
  await expect(picker.nth(0)).not.toBeChecked();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Create release", exact: true })
    .click();
  await expect(
    page.getByText("Development release 0.3.0 created."),
  ).toBeVisible();
  expect(releases[0].package_ids).toEqual(["package-newer"]);
  await page.getByRole("button", { name: "Promote", exact: true }).click();
  await page.getByLabel("Production version").fill("2.0.0");
  await page.getByRole("button", { name: "Promote release" }).click();
  await expect(
    page.getByText("Production release 2.0.0 is available."),
  ).toBeVisible();
  expect(releases[1].promoted_from).toBe(release.id);
});

test("publish review includes optional setup and webhook read failure does not block readiness", async ({
  page,
}) => {
  const changes = await mock(page, true, {
    tags: ["work", "tools"],
    visibility: "private",
    account_ids: ["c:alice"],
    domains: ["example.com"],
    logo: "https://example.com/logo.png",
    links: {
      website: "https://example.com",
      custom: [
        { label: "Community", url: "https://example.com/community", logo: "" },
      ],
    },
    latest_development: release,
    targets: ["macos-aarch64"],
  });
  await page.route("**/v1/apps/test-app/readiness", (route) =>
    reply(route, { ready: true, errors: [], required_commands: [] }),
  );
  await page.route("**/v1/apps/test-app/webhook", (route) =>
    reply(
      route,
      {
        error: {
          code: "accounts_unavailable",
          message: "Webhook configuration is temporarily unavailable.",
        },
      },
      503,
    ),
  );
  await page.goto("/developer/apps/test-app?step=7");
  await expect(page.locator(".review-details")).toContainText("work, tools");
  await expect(page.locator(".review-details")).toContainText("c:alice");
  await expect(page.locator(".review-details")).toContainText("example.com");
  await expect(page.locator(".review-details")).toContainText("Logo added");
  await expect(
    page.getByRole("link", {
      name: "Community: https://example.com/community",
    }),
  ).toBeVisible();
  await expect(
    page.getByText("Webhook configuration is temporarily unavailable."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Publish app", exact: true }).click();
  await expect(
    page.getByText("Your app is published and available in the store."),
  ).toBeVisible();
  expect(changes.some((x) => x.path.endsWith("/publish"))).toBe(true);
});

test("webhook retrieval failure never exposes editable defaults that could overwrite settings", async ({
  page,
}) => {
  await mock(page, true);
  await page.route("**/v1/apps/test-app/webhook", (route) =>
    reply(
      route,
      { error: { code: "unavailable", message: "Accounts is unavailable." } },
      503,
    ),
  );
  await page.goto("/developer/apps/test-app?step=6");
  await expect(page.getByText("Accounts is unavailable.")).toBeVisible();
  await expect(page.getByLabel("Webhook endpoint")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await page.getByRole("button", { name: "4 Links" }).click();
  await expect(
    page.getByRole("heading", { name: "Connect the rest of your app" }),
  ).toBeVisible();
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

test("online docs include a concrete publishing path and explain state and update behavior", async ({
  page,
}) => {
  await mock(page);
  await page.goto("/docs");
  for (const command of [
    "apps availability ring",
    "apps create ring --name Ring",
    "apps validate ./package",
    "apps pack ./package --output ./ring.tar.gz",
    "apps upload ring --target macos-aarch64 ./ring.tar.gz",
    "apps release ring --version 0.1.0 --package PACKAGE_ID",
    "apps promote ring DEVELOPMENT_RELEASE_ID --version 1.0.0",
    "apps publish ring",
  ])
    await expect(
      page.locator("code").filter({ hasText: command }).first(),
    ).toBeVisible();
  await expect(
    page.getByText(/An exact version selects the initial release/),
  ).toBeVisible();
  await expect(
    page.getByText(/Saving a home does not migrate files/),
  ).toBeVisible();
});

test("browser Back preserves a failed draft and leaves only after saving succeeds", async ({
  page,
}) => {
  const changes = await mock(page, true);
  let fail = true;
  await page.route("**/v1/apps?**", (route) =>
    reply(route, { items: [makeApp()], total: 1 }),
  );
  await page.route("**/v1/apps/test-app", (route) => {
    if (route.request().method() === "PATCH" && fail)
      return reply(
        route,
        {
          error: {
            code: "storage_unavailable",
            message: "Keep this draft until storage recovers.",
          },
        },
        503,
      );
    return route.fallback();
  });
  await page.goto("/store");
  await page.locator(".app-card").click();
  await page.getByRole("link", { name: "Manage app" }).click();
  await page
    .getByLabel("App name", { exact: true })
    .fill("Preserve browser history draft");
  await expect(page.getByText("Changes could not be saved")).toBeVisible();
  await page.goBack();
  await expect(page.getByLabel("App name", { exact: true })).toHaveValue(
    "Preserve browser history draft",
  );
  await expect(page).toHaveURL(/\/developer\/apps\/test-app$/);
  await expect(
    page.getByText("Keep this draft until storage recovers.").first(),
  ).toBeVisible();
  fail = false;
  await page.goBack();
  await expect(page).toHaveURL(/\/store\/test-app$/);
  await expect(
    page.getByRole("heading", { name: "Preserve browser history draft" }),
  ).toBeVisible();
  expect(
    changes.some(
      (change) => change.body.name === "Preserve browser history draft",
    ),
  ).toBe(true);
});
