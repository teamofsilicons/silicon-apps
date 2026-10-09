/** Search: /search?q=&tag=&target=&visibility=, a plain GET form, and the private apps view. */
import { expect, test } from "@playwright/test";
import { expectPageBasics, html, jsonLd, mainText, signIn, typesOf } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";

/** The app ids of the result cards, in order. */
function resultIds(page: string): string[] {
  const results = /<section aria-labelledby="results-title"[\s\S]*?<\/section>/.exec(page)?.[0] ?? "";
  return [...results.matchAll(/<a href="\/apps\/([^"]+)">/g)].map(match => decodeURIComponent(match[1]));
}

test("search ranks exact matches first and forgives a typo", async ({ request }) => {
  const exact = await html(request, "/search?q=briefcase");
  expect(resultIds(exact)[0]).toBe("briefcase");
  const typo = await html(request, "/search?q=brifcase");
  expect(resultIds(typo)).toContain("briefcase");
  const meta = expectPageBasics(exact, { canonical: `${ORIGIN}/search`, indexed: false });
  expect(meta.title).toBe("Apps matching “briefcase” · Silicon Apps");
  expect(typesOf(jsonLd(exact))).toEqual(expect.arrayContaining(["SearchResultsPage", "BreadcrumbList"]));
});

test("the full list, tag pages and platform pages are indexable collections", async ({ request }) => {
  const all = await html(request, "/search");
  expectPageBasics(all, { canonical: `${ORIGIN}/search` });
  expect(resultIds(all)).toEqual(expect.arrayContaining(["briefcase", "dm", "ring", "remind", "notes", "orbit", "ledgerly"]));
  expect(resultIds(all)).not.toContain("team-vault");
  expect(resultIds(all)).not.toContain("draft-thing");
  expect(typesOf(jsonLd(all))).toContain("CollectionPage");

  const tagged = await html(request, "/search?tag=productivity");
  expectPageBasics(tagged, { canonical: `${ORIGIN}/search?tag=productivity` });
  expect(resultIds(tagged).sort()).toEqual(["briefcase", "notes", "remind"]);

  // Ring has no Windows package, so it is not in the Windows list.
  const windows = await html(request, "/search?target=windows-x86_64");
  expect(resultIds(windows)).toContain("briefcase");
  expect(resultIds(windows)).not.toContain("ring");
  expect(mainText(windows)).toContain("Apps for Windows x64");

  const combined = await html(request, "/search?q=ring&tag=alerts&target=linux-x86_64&visibility=public");
  expect(resultIds(combined)).toEqual(["ring"]);
});

test("a search with no results says what to try", async ({ request }) => {
  const page = await html(request, "/search?q=zzzzqqqq");
  expect(resultIds(page)).toEqual([]);
  expect(mainText(page)).toContain("No apps match that");
  expect(mainText(page)).toContain("Clear search and filters");
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("the filters are one GET form", async ({ page }) => {
    await page.goto("/search");
    const form = page.getByRole("search", { name: "Search and filter apps" });
    await form.getByLabel("Search", { exact: true }).fill("notes");
    await form.getByLabel("Tag").selectOption("writing");
    await form.getByLabel("Platform").selectOption("linux-x86_64");
    await form.getByRole("radio", { name: "Public" }).check();
    await form.getByRole("button", { name: "Show apps" }).click();
    await expect(page).toHaveURL(/\/search\?q=notes&tag=writing&target=linux-x86_64&visibility=public$/);
    await expect(page.getByRole("link", { name: "Field Notes" })).toBeVisible();
    await expect(page.getByRole("status").filter({ hasText: "1 app" })).toBeVisible();
  });
});

test("signed out, the private apps view says to log in", async ({ request, page }) => {
  const source = await html(request, "/search?visibility=private");
  const text = mainText(source);
  expect(text).toContain("Log in to see private apps");
  expect(text).not.toContain("Team Vault");
  await page.goto("/search?visibility=private");
  const signInLink = page.getByRole("link", { name: "Sign in with Silicon Accounts" });
  await expect(signInLink).toHaveAttribute("href", `/sign-in?return_to=${encodeURIComponent("/search?visibility=private")}`);
});

test("signed in, the private apps view shows only what is shared with you", async ({ browser, baseURL }) => {
  const mira = await browser.newContext({ baseURL });
  await signIn(mira, baseURL!, "mira");
  const miraPage = await mira.newPage();
  await miraPage.goto("/search?visibility=private");
  await expect(miraPage.getByRole("link", { name: "Team Vault" })).toBeVisible();
  await expect(miraPage.getByText("Private", { exact: true }).first()).toBeVisible();
  await miraPage.goto("/apps/team-vault");
  await expect(miraPage.getByRole("heading", { level: 1, name: "Team Vault" })).toBeVisible();
  await mira.close();

  const nova = await browser.newContext({ baseURL });
  await signIn(nova, baseURL!, "nova");
  const novaPage = await nova.newPage();
  await novaPage.goto("/search?visibility=private");
  await expect(novaPage.getByRole("heading", { name: "No private apps yet" })).toBeVisible();
  const response = await novaPage.goto("/apps/team-vault");
  expect(response?.status()).toBe(404);
  await nova.close();
});
