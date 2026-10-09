/**
 * Signing in and out through Silicon Accounts (the API's browser routes), and reviews: 1 to 5 stars and up to 600
 * characters, as a plain form that works with no script.
 */
import { expect, test } from "@playwright/test";
import { SESSIONS, signIn } from "./helpers";

test("Sign in goes through the Apps API to Silicon Accounts and comes back to the same page", async ({ page, request }) => {
  await page.goto("/apps/briefcase");
  const header = page.getByRole("banner");
  await expect(header.getByRole("link", { name: "Sign in" })).toHaveAttribute("href", `/sign-in?return_to=${encodeURIComponent("/apps/briefcase")}`);

  const start = await request.get(`/sign-in?return_to=${encodeURIComponent("/apps/briefcase#reviews")}`, { maxRedirects: 0 });
  expect(start.status()).toBe(303);
  expect(start.headers().location).toBe(`/v1/auth/login?return_to=${encodeURIComponent("/apps/briefcase#reviews")}`);

  // The API owns PKCE, state and the session cookie, and signs in as the app silicon-apps.
  const login = await request.get(start.headers().location, { maxRedirects: 0 });
  expect([302, 303, 307]).toContain(login.status());
  const authorize = new URL(login.headers().location);
  expect(authorize.pathname).toBe("/authorize");
  expect(authorize.searchParams.get("app_id")).toBe("silicon-apps");
  expect(authorize.searchParams.get("code_challenge_method")).toBe("S256");
  expect(authorize.searchParams.get("redirect_uri")).toMatch(/\/v1\/auth\/callback$/);
  expect(login.headers()["set-cookie"]).toContain("apps_oauth_state=");

  // Only local paths come back: anything else returns to the home page.
  const outside = await request.get(`/sign-in?return_to=${encodeURIComponent("https://example.com/")}`, { maxRedirects: 0 });
  expect(outside.headers().location).toBe(`/v1/auth/login?return_to=${encodeURIComponent("/")}`);
});

test("signed in, the header shows the account; Sign out ends the session", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL });
  await signIn(context, baseURL!, "ada");
  const page = await context.newPage();
  await page.goto("/");
  await expect(page.getByRole("link", { name: "Settings, signed in as c:ada" })).toBeVisible();
  await page.goto("/settings");
  await expect(page.getByRole("region", { name: "Silicon Accounts" })).toContainText("c:ada · Carbon");
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole("banner").getByRole("link", { name: "Sign in" })).toBeVisible();
  expect((await context.cookies()).find(cookie => cookie.name === "apps_session")).toBeUndefined();
  await context.close();
});

test("a sign-out posted from another site is refused", async ({ request }) => {
  const response = await request.post("/sign-out", { headers: { Origin: "https://example.com", Cookie: `apps_session=${SESSIONS.mira}` }, form: { return_to: "/" } });
  expect(response.status()).toBe(403);
  expect((await response.json()).error.code).toBe("origin_mismatch");
});

test("signed out, the app page asks you to sign in to review", async ({ page }) => {
  await page.goto("/apps/ledgerly");
  const link = page.getByRole("link", { name: "Sign in to write a review" });
  await expect(link).toHaveAttribute("href", `/sign-in?return_to=${encodeURIComponent("/apps/ledgerly#reviews")}`);
  await expect(page.locator("form textarea[name=text]")).toHaveCount(0);
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("write, edit and remove a review with plain forms", async ({ browser, baseURL }) => {
    const context = await browser.newContext({ baseURL, javaScriptEnabled: false });
    await signIn(context, baseURL!, "nova");
    const page = await context.newPage();
    await page.goto("/apps/ledgerly");
    const form = page.locator("form").filter({ has: page.locator("textarea[name=text]") });
    await expect(form.getByRole("radio")).toHaveCount(5);
    await expect(form.locator("textarea")).toHaveAttribute("maxlength", "600");
    await form.getByRole("radio", { name: "4 stars" }).check();
    await form.locator("textarea").fill("Scripted bookkeeping that balances every time.");
    await form.getByRole("button", { name: /Post review|Save changes/ }).click();
    await expect(page).toHaveURL(/\/apps\/ledgerly\?review=saved#reviews$/);
    await expect(page.getByText("Your review is saved.")).toBeVisible();
    const mine = page.getByRole("article", { name: "Your review" });
    await expect(mine).toContainText("Scripted bookkeeping that balances every time.");
    await expect(mine.getByRole("img", { name: "4.0 out of 5 stars" })).toBeVisible();

    // Edit: the form now holds the review.
    await expect(form.getByRole("heading", { name: "Edit your review" })).toBeVisible();
    await expect(form.locator("input[name=rating][value='4']")).toBeChecked();
    await form.getByRole("radio", { name: "5 stars" }).check();
    await form.getByRole("button", { name: "Save changes" }).click();
    await expect(page.getByRole("article", { name: "Your review" }).getByRole("img", { name: "5.0 out of 5 stars" })).toBeVisible();

    // Remove it.
    await page.getByText("Remove your review").click();
    await page.getByRole("button", { name: "Remove my review" }).click();
    await expect(page).toHaveURL(/review=removed/);
    await expect(page.getByText("Your review is removed.")).toBeVisible();
    await expect(page.getByRole("article", { name: "Your review" })).toHaveCount(0);
    await context.close();
  });
});

test("a review longer than 600 characters is refused and nothing is saved", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL });
  await signIn(context, baseURL!, "mira");
  const page = await context.newPage();
  await page.goto("/apps/silicon-accounts");
  const form = page.locator("form").filter({ has: page.locator("textarea[name=text]") });
  await expect(form.getByText("600 characters left")).toBeVisible();
  // Around the browser's own limit, as a script or an old browser could send it.
  await form.locator("textarea").evaluate(field => field.removeAttribute("maxlength"));
  await form.locator("textarea").fill("x".repeat(601));
  await form.getByRole("radio", { name: "3 stars" }).check();
  await form.getByRole("button", { name: /Post review|Save changes/ }).click();
  await expect(page).toHaveURL(/review=error&code=too_long/);
  await expect(page.getByText("Keep your review to 600 characters or fewer.")).toBeVisible();
  await expect(page.getByRole("article", { name: "Your review" })).toHaveCount(0);
  await context.close();
});
