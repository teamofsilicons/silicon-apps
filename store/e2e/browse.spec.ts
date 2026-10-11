/** Browsing: the home page, author pages and old store addresses, as HTML a crawler reads and as a page with no script. */
import { expect, test } from "@playwright/test";
import { expectPageBasics, html, jsonLd, mainText, typesOf } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";

test("the home page is server-rendered with what Silicon Apps is, search and the apps", async ({ request }) => {
  const page = await html(request, "/");
  // Next writes the root's canonical address without its trailing slash; both name the same page.
  const meta = expectPageBasics(page, { canonical: ORIGIN });
  expect(meta.title).toBe("Silicon Apps: apps for Carbons and Silicons");
  const text = mainText(page);
  expect(text).toContain("Apps for Carbons and Silicons");
  expect(text).toContain("Every app here is a command line app first");
  for (const section of ["Apps", "Get the CLI", "For Silicons"]) expect(text).toContain(section);
  // No questions section: the store explains itself in its own lines, and Silicons read /llms.txt.
  expect(text).not.toContain("Questions, answered");
  expect(text).toContain("Make a Silicon App");
  expect(text).toContain("add Silicon Accounts to your app");
  // Every public app is listed; drafts and private apps never are.
  for (const name of ["Briefcase", "DM", "Ring", "Field Notes", "Orbit", "Ledgerly"]) expect(text).toContain(name);
  expect(text).not.toContain("Team Vault");
  expect(text).not.toContain("Draft Thing");
  // A plain GET search form.
  const searchForm = (page.match(/<form [^>]*>/g) ?? []).find(tag => tag.includes('role="search"'));
  expect(searchForm).toContain('action="/search"');
  expect(searchForm).toContain('method="get"');
  // Tags are never links or filters; the list ends with a way to every app.
  expect(page).not.toContain("/search?tag=");
  expect(text).toContain("View all 9 apps");
  // "Make a Silicon App" and "Add Silicon Accounts to your app" point to the developer portal.
  expect(page).toContain('href="https://developers.teamofsilicons.com/docs/apps"');
  expect(page).toContain('href="https://developers.teamofsilicons.com/docs/accounts"');
  const nodes = jsonLd(page);
  expect(typesOf(nodes)).toEqual(expect.arrayContaining(["Organization", "WebSite", "WebPage", "ItemList"]));
  expect(typesOf(nodes)).not.toContain("FAQPage");
  const website = nodes.find(node => node["@type"] === "WebSite") as { potentialAction: { "@type": string; target: { urlTemplate: string } } };
  expect(website.potentialAction["@type"]).toBe("SearchAction");
  expect(website.potentialAction.target.urlTemplate).toBe(`${ORIGIN}/search?q={search_term_string}`);
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("the home page works and its search is a plain form", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1, name: "Apps for Carbons and Silicons" })).toBeVisible();
    await expect(page.getByRole("heading", { level: 2, name: "Apps" })).toBeVisible();
    await page.getByRole("searchbox", { name: "Search apps" }).first().fill("briefcase");
    await page.getByRole("button", { name: "Search", exact: true }).first().click();
    await expect(page).toHaveURL(/\/search\?q=briefcase$/);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Apps matching “briefcase”");
    await page.goto("/");
    await page.getByRole("link", { name: /View all 9 apps/ }).click();
    await expect(page).toHaveURL(/\/search$/);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Apps");
  });

  test("an app card opens the app's page", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("link", { name: "Briefcase", exact: true }).first().click();
    await expect(page).toHaveURL(/\/apps\/briefcase$/);
    await expect(page.getByRole("heading", { level: 1, name: "Briefcase" })).toBeVisible();
  });
});

test("an author's page lists their published apps", async ({ request, page }) => {
  const source = await html(request, "/authors/shubham");
  expectPageBasics(source, { canonical: `${ORIGIN}/authors/shubham` });
  const text = mainText(source);
  expect(text).toContain("Shubham");
  expect(text).toContain("c:shubham");
  expect(text).toContain("Briefcase");
  expect(text).toContain("Remind");
  expect(typesOf(jsonLd(source))).toEqual(expect.arrayContaining(["ProfilePage", "BreadcrumbList"]));
  await page.goto("/apps/briefcase");
  await page.getByRole("region", { name: "Authors" }).getByRole("link", { name: /Shubham/ }).click();
  await expect(page).toHaveURL(/\/authors\/shubham$/);
  expect((await request.get("/authors/nobody-here")).status()).toBe(404);
});

test("old store addresses keep working", async ({ request }) => {
  const expectations: Array<[string, string]> = [
    ["/store", "/"],
    ["/store?q=brief", "/search?q=brief"],
    ["/store?visibility=private", "/search?visibility=private"],
    ["/store/briefcase", "/apps/briefcase"],
    ["/apps", "/search"],
    ["/docs", "https://developers.teamofsilicons.com/docs/apps"],
    ["/developer/apps/briefcase", "https://developers.teamofsilicons.com/apps/briefcase/publishing"],
  ];
  for (const [from, to] of expectations) {
    const response = await request.get(from, { maxRedirects: 0 });
    expect(response.status(), from).toBe(308);
    const location = response.headers().location;
    expect(location.startsWith("http") && !to.startsWith("http") ? new URL(location).pathname + new URL(location).search : location, from).toBe(to);
  }
});

test("unknown pages answer 404 with a way back", async ({ request }) => {
  const response = await request.get("/no-such-page");
  expect(response.status()).toBe(404);
  const text = mainText(await response.text());
  expect(text).toContain("This page is not here");
});

test("no page scrolls sideways at 320 pixels", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ viewport: { width: 320, height: 800 }, baseURL });
  const page = await context.newPage();
  for (const path of ["/", "/search", "/search?q=productivity", "/apps/briefcase", "/apps/orbit", "/authors/shubham", "/settings", "/search?visibility=private"]) {
    await page.goto(path);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow, path).toBe(0);
  }
  await context.close();
});
