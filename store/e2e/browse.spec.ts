/** Browsing: the home page, All Apps, author pages, the 404 and old store addresses, as HTML a crawler reads and as a page with no script. */
import { expect, test } from "@playwright/test";
import { AUTHORS, expectPageBasics, html, jsonLd, mainText, metaOf, typesOf } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";
const PUBLIC_APPS = ["briefcase", "dm", "ring", "remind", "notes", "orbit", "ledgerly", "silicon-accounts", "silicon-apps"];

test("the home page is server-rendered with the core apps, the categories and search", async ({ request }) => {
  const page = await html(request, "/");
  const meta = expectPageBasics(page);
  expect(meta.title).toBe("Silicon Apps: apps for Carbons and Silicons");
  expect(meta.description).toContain("Every app is a command line app first");
  expect(page).toMatch(/<h1 class="sr-only">Silicon Apps<\/h1>/);
  const text = mainText(page);
  // The banner: a terminal installing an app.
  expect(text).toContain("$ silicon-apps install briefcase");
  // Core Apps to Try: Briefcase, DM and Ring, in that order, each a link to its page.
  expect(text).toMatch(/Core Apps to Try Briefcase .* DM .* Ring /);
  for (const id of ["briefcase", "dm", "ring"]) expect(page).toContain(`href="/apps/${id}"`);
  // The categories, each a link to its own page (Essentials, which every visitor already has, is left out).
  expect(text).toContain("Categories");
  for (const id of ["communication", "productivity", "business"]) expect(page).toContain(`href="/categories/${id}"`);
  // Drafts and private apps are never shown.
  expect(text).not.toContain("Team Vault");
  expect(text).not.toContain("Draft Thing");
  // A plain GET search form in the sidebar, to the home page.
  const searchForm = (page.match(/<form [^>]*>/g) ?? []).find(tag => tag.includes('role="search"'));
  expect(searchForm).toContain('action="/"');
  expect(searchForm).toContain('method="get"');
  expect(page).toMatch(/<input [^>]*name="q"/);
  // The sidebar's sections and the developer portal.
  expect(page).toContain('href="/apps"');
  expect(page).toContain('href="https://developers.teamofsilicons.com"');
  expect(page).toContain('href="https://developers.teamofsilicons.com/docs/apps"');
  // Tags are never links or filters.
  expect(page).not.toContain("?tag=");
  const website = jsonLd(page).find(node => node["@type"] === "WebSite") as { potentialAction: { "@type": string; target: { urlTemplate: string } } };
  expect(website.potentialAction["@type"]).toBe("SearchAction");
  expect(website.potentialAction.target.urlTemplate).toBe(`${ORIGIN}/?q={search_term_string}`);
});

test("All Apps lists every public app by category, and never drafts or private apps", async ({ request }) => {
  const page = await html(request, "/apps");
  const meta = expectPageBasics(page, { canonical: `${ORIGIN}/apps` });
  expect(meta.title).toBe("All Apps · Silicon Apps");
  const text = mainText(page);
  expect(text).toContain(`All Apps ${PUBLIC_APPS.length} apps`);
  const listed = [...page.matchAll(/<a [^>]*href="\/apps\/([^"/]+)"/g)].map(match => match[1]);
  for (const id of PUBLIC_APPS) expect(listed, id).toContain(id);
  expect(listed).not.toContain("team-vault");
  expect(listed).not.toContain("draft-thing");
  // Each category is a chapter with a link to its own page, and a jump link at the top.
  for (const [id, title] of [["essentials", "Essentials"], ["communication", "Communication"], ["productivity", "Productivity"], ["business", "Business"]]) {
    expect(page).toMatch(new RegExp(`<section id="${id}"[^>]*aria-labelledby="${id}-title"`));
    expect(page).toContain(`href="#${id}"`);
    expect(page).toContain(`href="/categories/${id}"`);
    expect(text).toContain(title);
  }
  expect(text).toContain("Communication Messages, calls, notifications and contacts. See all 2 DM");
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("search is a plain form and its results open the app's page", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("region", { name: "Core Apps to Try" })).toBeVisible();
    const search = page.getByRole("search", { name: "Search apps" }).first();
    await search.getByRole("searchbox", { name: "Search apps" }).fill("briefcase");
    await search.getByRole("searchbox", { name: "Search apps" }).press("Enter");
    await expect(page).toHaveURL(/\/\?q=briefcase$/);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Apps matching “briefcase”");
    // The field keeps the search.
    await expect(page.getByRole("searchbox", { name: "Search apps" }).first()).toHaveValue("briefcase");
    await page.getByRole("list", { name: "Apps matching briefcase" }).getByRole("link", { name: /Briefcase/ }).click();
    await expect(page).toHaveURL(/\/apps\/briefcase$/);
    await expect(page.getByRole("heading", { level: 1, name: "Briefcase" })).toBeVisible();
  });

  test("a core app's card on the home page opens the app's page", async ({ page }) => {
    await page.goto("/");
    await page.locator("main").getByRole("link", { name: /^Ring/ }).click();
    await expect(page).toHaveURL(/\/apps\/ring$/);
    await expect(page.getByRole("heading", { level: 1, name: "Ring" })).toBeVisible();
  });
});

test("an author's page lists their published apps", async ({ request, page }) => {
  const path = `/authors/${AUTHORS.shubham}`;
  const source = await html(request, path);
  const meta = expectPageBasics(source, { canonical: `${ORIGIN}${path}` });
  expect(meta.title).toBe("Shubham · Silicon Apps");
  expect(source).toMatch(/<h1 id="author-name"[^>]*>Shubham</);
  const text = mainText(source);
  for (const expected of ["c:shubham", "Carbon", "Apps 2", "Systems macOS Linux Windows", "Works with", "Head of Growth", "Publishing since", "Briefcase", "Remind"]) {
    expect(text, expected).toContain(expected);
  }
  // Drafts and private apps by the same author never show.
  expect(text).not.toContain("Team Vault");
  // Works with links to the co-author's page.
  expect(source).toContain(`href="/authors/${AUTHORS.growth}"`);
  expect(typesOf(jsonLd(source))).toContain("ProfilePage");

  // The app page's developer card leads here.
  await page.goto("/apps/briefcase");
  await page.getByRole("complementary", { name: "Get it" }).getByRole("link", { name: /Shubham/ }).first().click();
  await expect(page).toHaveURL(new RegExp(`/authors/${AUTHORS.shubham}$`));
  await expect(page.getByRole("heading", { level: 1, name: "Shubham" })).toBeVisible();

  // Unknown authors, and ids that are not UUIDs, answer 404.
  for (const missing of ["/authors/nobody-here", "/authors/5a4e7c10-0000-4000-8000-00000000ffff"]) {
    const response = await request.get(missing);
    expect(response.status(), missing).toBe(404);
    expect(mainText(await response.text()), missing).toContain("This page drifted off.");
  }
});

test("old store addresses keep working", async ({ request }) => {
  const expectations: Array<[string, string]> = [
    ["/store", "/"],
    ["/store?q=brief", "/?q=brief"],
    ["/search?q=brief", "/?q=brief"],
    ["/store?visibility=private", "/?visibility=private"],
    ["/store/briefcase", "/apps/briefcase"],
    ["/store/apps", "/apps/silicon-apps"],
    ["/apps/apps", "/apps/silicon-apps"],
    // /apps is All Apps now; an old /apps search link still lands on the home page's search.
    ["/apps?q=brief", "/?q=brief"],
    ["/docs", "https://developers.teamofsilicons.com/docs/apps"],
    ["/developer/apps/briefcase", "https://developers.teamofsilicons.com/apps/briefcase/publishing"],
  ];
  for (const [from, to] of expectations) {
    const response = await request.get(from, { maxRedirects: 0 });
    expect(response.status(), from).toBe(308);
    const location = response.headers().location;
    expect(location.startsWith("http") && !to.startsWith("http") ? new URL(location).pathname + new URL(location).search : location, from).toBe(to);
  }
  // /apps itself is a page, not a redirect.
  expect((await request.get("/apps", { maxRedirects: 0 })).status()).toBe(200);
});

test("unknown pages answer 404 with a way back and credit the art on the wall", async ({ request, page }) => {
  const response = await request.get("/no-such-page");
  expect(response.status()).toBe(404);
  const source = await response.text();
  const meta = metaOf(source);
  expect(meta.title).toBe("Page Not Found · Silicon Apps");
  expect(meta.robots).toContain("noindex");
  const text = mainText(source);
  expect(text).toContain("404");
  expect(text).toContain("This page drifted off.");

  await page.goto("/no-such-page");
  // The visitor keeps the address they asked for, and the page is the store's, with its sidebar.
  await expect(page).toHaveURL(/\/no-such-page$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("404 This page drifted off.");
  const main = page.locator("main");
  // The artwork is whole, described, and credited with a link to the work.
  const art = main.getByRole("figure");
  await expect(art.getByRole("img")).toHaveAttribute("alt", /hot-air balloon/);
  await expect(art.getByRole("link", { name: "Untitled II, Austin Augie" })).toHaveAttribute("href", "https://www.austinaugie.nyc/product-page/untitled-ii-1126");
  await main.getByRole("link", { name: "Go Home" }).click();
  await expect(page).toHaveURL(/\/$/);
});

test("no page scrolls sideways at 320 pixels", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ viewport: { width: 320, height: 800 }, baseURL });
  const page = await context.newPage();
  for (const path of ["/", "/?q=productivity", "/apps", "/categories/productivity", "/apps/briefcase", "/apps/orbit", "/apps/ring", `/authors/${AUTHORS.shubham}`, "/no-such-page"]) {
    await page.goto(path);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow, path).toBe(0);
  }
  await context.close();
});
