import { type Page } from "@playwright/test";
import type { App } from "../src/types";
export const account = {
  uuid: "test-author",
  id: "c:author",
  display_name: "Author",
};
export const makeApp = (): App => ({
  app_id: "test-app",
  name: "A useful app",
  description:
    "A useful tool for keeping your ideas close and your work organized. Build small routines, connect your tools, and bring your projects together with a command line made for Carbons and Silicons. It fits into your existing workflow with clear help and dependable commands.",
  logo: "",
  logo_alt: "",
  banner: "",
  banner_alt: "",
  tags: ["productivity"],
  visibility: "public",
  domains: [],
  account_ids: [],
  links: {},
  carousel: [],
  published: false,
  setup_step: 1,
  created_at: "2026-10-08T12:00:00Z",
  updated_at: "2026-10-08T12:00:00Z",
  authors: [{ ...account, joined_at: "2026-10-08T12:00:00Z" }],
  targets: [],
  latest_production: null,
  latest_development: null,
  rating: null,
  review_count: 0,
  installs: 0,
  is_author: true,
  is_admin: true,
});
export async function mock(
  page: Page,
  signedIn = false,
  initial: Partial<ReturnType<typeof makeApp>> = {},
) {
  let app = { ...makeApp(), ...initial };
  const mutations: {
    path: string;
    body: Record<string, unknown>;
    headers: Record<string, string>;
  }[] = [];
  await page.route("**/v1/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname.replace("/v1", "");
    const method = request.method();
    let body = {};
    try {
      body = request.postDataJSON() || {};
    } catch {}
    if (method !== "GET")
      mutations.push({ path, body, headers: request.headers() });
    let result: unknown = {};
    let status = 200;
    if (path === "/session")
      result = { authenticated: signedIn, account: signedIn ? account : null };
    else if (path === "/telemetry") result = { status: "queued" };
    else if (path === "/apps" && method === "GET")
      result = { items: [], total: 0 };
    else if (path.startsWith("/apps/availability/"))
      result = { available: true };
    else if (path === "/apps" && method === "POST") {
      app = { ...app, ...body };
      result = { app, app_secret: "test-secret-shown-once" };
    } else if (path === "/apps/test-app") {
      if (method === "PATCH") app = { ...app, ...body };
      result = app;
    } else if (path === "/apps/test-app/access") {
      app = { ...app, ...body };
      result = app;
    } else if (path === "/apps/test-app/packages/macos-aarch64") {
      status = 422;
      result = {
        error: {
          code: "validation_failed",
          message: "accounts --json returned the wrong app_id.",
          hint: "Return test-app in the app_id field.",
          details: {
            command: "accounts --json",
            expected: "test-app",
            actual: "wrong-app",
            stderr: "contract mismatch",
          },
        },
      };
    } else if (path === "/apps/test-app/readiness")
      result = {
        ready: false,
        errors: [
          {
            field: "packages",
            message: "Upload a validated package and create a release.",
          },
        ],
        required_commands: [],
      };
    else if (path === "/apps/test-app/reviews")
      result = { items: [], rating: null, count: 0 };
    else if (path === "/targets")
      result = { items: [], total_population: null, total_reach: null };
    else if (path === "/apps/test-app/webhook") result = { configured: false };
    else if (path === "/apps/test-app/review")
      result = {
        uuid: account.uuid,
        rating: 5,
        text: "Useful",
        id: account.id,
      };
    else if (
      path.endsWith("/packages") ||
      path.endsWith("/releases") ||
      path.endsWith("/invites") ||
      path === "/invites"
    )
      result = { items: [] };
    else if (path.endsWith("/authors")) result = { items: app.authors };
    else if (path.endsWith("/history")) result = { items: [], total: 0 };
    else if (path === "/auth/logout") result = { authenticated: false };
    await route.fulfill({
      status,
      contentType: "application/json",
      body: JSON.stringify(result),
    });
  });
  return mutations;
}
