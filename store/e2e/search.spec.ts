/** Search on the home page: /?q=&target=&visibility=, a plain GET form, and the private apps view. Tags are matched by the search text, never filtered by. */
import { expect, test } from "@playwright/test";
import { expectPageBasics, html, jsonLd, mainText, signIn, typesOf } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";

/** The app ids of the result cards, in order. */
function resultIds(page: string): string[] {
  const results = /<section aria-labelledby="results-title"[\s\S]*?<\/section>/.exec(page)?.[0] ?? "";
  return [...results.matchAll(/<a href="\/apps\/([^"]+)">/g)].map(match => decodeURIComponent(match[1]));
}

test("search ranks exact matches first and forgives a typo", async ({ request }) => {
  const exact = await html(request, "/?q=briefcase");
  expect(resultIds(exact)[0]).toBe("briefcase");
  const typo = await html(request, "/?q=brifcase");
  expect(resultIds(typo)).toContain("briefcase");
  const meta = expectPageBasics(exact, { canonical: ORIGIN, indexed: false });
  expect(meta.title).toBe("Apps matching “briefcase” · Silicon Apps");
  expect(typesOf(jsonLd(exact))).toEqual(expect.arrayContaining(["SearchResultsPage"]));
});

test("the full list on the home page and the platform lists are indexable", async ({ request }) => {
  const all = await html(request, "/");
  expectPageBasics(all, { canonical: ORIGIN });
  expect(resultIds(all)).toEqual(expect.arrayContaining(["briefcase", "dm", "ring", "remind", "notes", "orbit", "ledgerly"]));
  expect(resultIds(all)).not.toContain("team-vault");
  expect(resultIds(all)).not.toContain("draft-thing");
  expect(typesOf(jsonLd(all))).toContain("WebPage");

  // A tag is found through the search text; an old tag address moves to that search.
  const byTag = await html(request, "/?q=productivity");
  expect(resultIds(byTag)).toEqual(expect.arrayContaining(["briefcase", "notes", "remind"]));
  const old = await request.get("/?tag=productivity", { maxRedirects: 0 });
  expect(old.status()).toBe(308);
  expect(old.headers().location).toBe("/?q=productivity");

  // Ring has no Windows package, so it is not in the Windows list.
  const windows = await html(request, "/?target=windows-x86_64");
  expect(resultIds(windows)).toContain("briefcase");
  expect(resultIds(windows)).not.toContain("ring");
  expect(mainText(windows)).toContain("Apps for Windows x64");

  const combined = await html(request, "/?q=ring&target=linux-x86_64&visibility=public");
  expect(resultIds(combined)).toEqual(["ring"]);
});

test("the additional filters narrow and sort the list, and the address stays tidy", async ({ request }) => {
  const popular = await html(request, "/?installs=1000");
  expect(resultIds(popular).sort()).toEqual(["briefcase", "dm"]);
  const rated = await html(request, "/?rating=4.5");
  expect(resultIds(rated)).toEqual(["briefcase"]);
  const sorted = await html(request, "/?sort=installs");
  expect(resultIds(sorted)[0]).toBe("briefcase");
  // Filtered views point search engines at the plain list and are not indexed themselves.
  expectPageBasics(popular, { canonical: ORIGIN, indexed: false });
  // The form submits empty and default values; the page answers with the tidy address.
  const messy = await request.get("/?q=&target=&visibility=all&added=&installs=&sort=", { maxRedirects: 0 });
  expect(messy.status()).toBe(307);
  expect(messy.headers().location).toBe("/");
});

test("a search with no results says what to try", async ({ request }) => {
  const page = await html(request, "/?q=zzzzqqqq");
  expect(resultIds(page)).toEqual([]);
  expect(mainText(page)).toContain("No apps match that");
  expect(mainText(page)).toContain("Clear search and filters");
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("the filters are one GET form", async ({ page }) => {
    await page.goto("/");
    const form = page.getByRole("search", { name: "Search and filter apps" });
    await form.getByLabel("Search apps", { exact: true }).fill("notes");
    // The additional filters wait behind a native disclosure, closed until one is set.
    await expect(form.getByLabel("Platform")).toBeHidden();
    await form.locator("summary").filter({ hasText: "Filters" }).click();
    await form.getByLabel("Platform").selectOption("linux-x86_64");
    await form.getByRole("radio", { name: "Public" }).check();
    await form.getByRole("button", { name: "Show apps" }).click();
    await expect(page).toHaveURL(/\/\?q=notes&target=linux-x86_64&visibility=public$/);
    await expect(page.getByRole("link", { name: "Field Notes" })).toBeVisible();
    await expect(page.getByRole("status").filter({ hasText: "1 app" })).toBeVisible();
  });
});

test("signed out, the private apps view says to log in", async ({ request, page }) => {
  const source = await html(request, "/?visibility=private");
  const text = mainText(source);
  expect(text).toContain("Log in to see private apps");
  expect(text).not.toContain("Team Vault");
  await page.goto("/?visibility=private");
  const signInLink = page.getByRole("link", { name: "Sign in with Silicon Accounts" });
  await expect(signInLink).toHaveAttribute("href", `/sign-in?return_to=${encodeURIComponent("/?visibility=private")}`);
});

test("signed in, the private apps view shows only what is shared with you", async ({ browser, baseURL }) => {
  const mira = await browser.newContext({ baseURL });
  await signIn(mira, baseURL!, "mira");
  const miraPage = await mira.newPage();
  await miraPage.goto("/?visibility=private");
  await expect(miraPage.getByRole("link", { name: "Team Vault" })).toBeVisible();
  await expect(miraPage.getByText("Private", { exact: true }).first()).toBeVisible();
  await miraPage.goto("/apps/team-vault");
  await expect(miraPage.getByRole("heading", { level: 1, name: "Team Vault" })).toBeVisible();
  await mira.close();

  const nova = await browser.newContext({ baseURL });
  await signIn(nova, baseURL!, "nova");
  const novaPage = await nova.newPage();
  await novaPage.goto("/?visibility=private");
  await expect(novaPage.getByRole("heading", { name: "No private apps yet" })).toBeVisible();
  const response = await novaPage.goto("/apps/team-vault");
  expect(response?.status()).toBe(404);
  await nova.close();
});

test("with script, each filter is a quiet dropdown: one click to a value, one click back to any", async ({ page }) => {
  await page.goto("/");
  const bar = page.getByRole("group", { name: "Filters" });
  await bar.getByRole("button", { name: "Installs" }).click();
  await page.getByRole("menuitem", { name: "1,000 or more" }).click();
  await expect(page).toHaveURL(/\/\?installs=1000$/);
  await expect(page.getByRole("status").filter({ hasText: "2 apps" })).toBeVisible();
  await bar.getByRole("button", { name: "1,000 or more" }).click();
  await page.getByRole("menuitem", { name: "Any number of installs" }).click();
  await expect(page).toHaveURL(/\/$/);
  await bar.getByRole("button", { name: /Sort: Best match/ }).click();
  await page.getByRole("menuitem", { name: "Most installed" }).click();
  await expect(page).toHaveURL(/\/\?sort=installs$/);
});

test("the rating filter is five stars: the keyboard moves by half a star and Enter keeps it", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("group", { name: "Filters" }).getByRole("button", { name: "Rating" }).click();
  const stars = page.getByRole("slider", { name: "Minimum rating" });
  await stars.focus();
  await stars.press("End");
  await stars.press("ArrowLeft");
  await expect(stars).toHaveAttribute("aria-valuetext", "4.5 stars or more");
  await stars.press("Enter");
  await expect(page).toHaveURL(/\/\?rating=4\.5$/);
  await expect(page.getByRole("status").filter({ hasText: "1 app" })).toBeVisible();
});
