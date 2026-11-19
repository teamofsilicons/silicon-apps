/** Search on the home page: /?q=, a plain GET form in the sidebar. Tags and descriptions are matched by the search text; matching categories are linked above the results. */
import { expect, test } from "@playwright/test";
import { expectPageBasics, html, mainText } from "./helpers";

/** The app ids of the result rows, in order. */
function resultIds(page: string): string[] {
  const results = /<ul [^>]*aria-label="Apps matching [^"]*"[\s\S]*?<\/ul>/.exec(page)?.[0] ?? "";
  return [...results.matchAll(/<a href="\/apps\/([^"]+)"/g)].map(match => decodeURIComponent(match[1]));
}

test("search ranks exact and prefix matches first and forgives a typo", async ({ request }) => {
  const exact = await html(request, "/?q=briefcase");
  expectPageBasics(exact);
  expect(resultIds(exact)).toEqual(["briefcase"]);
  const text = mainText(exact);
  expect(text).toContain("1 app");
  expect(text).toContain("Keeps your files in one place for you and your Silicons.");
  expect(text).toContain("4.5 ★");
  // The search box keeps what was searched for.
  expect(exact).toMatch(/<input [^>]*name="q"[^>]*value="briefcase"|<input [^>]*value="briefcase"[^>]*name="q"/);

  const typo = await html(request, "/?q=brifcase");
  expect(resultIds(typo)).toEqual(["briefcase"]);

  // A name that starts with the search comes before apps that only mention it.
  const prefix = resultIds(await html(request, "/?q=led"));
  expect(prefix[0]).toBe("ledgerly");
  expect(prefix.length).toBeGreaterThan(1);
  const silicon = resultIds(await html(request, "/?q=silicon"));
  expect(silicon.slice(0, 2).sort()).toEqual(["silicon-accounts", "silicon-apps"]);
  expect(silicon.length).toBeGreaterThan(2);
  // Drafts and private apps are never found.
  expect(silicon).not.toContain("team-vault");
  expect(silicon).not.toContain("draft-thing");
});

test("a tag is found through the search text, with the matching category linked", async ({ request }) => {
  const page = await html(request, "/?q=productivity");
  expect(resultIds(page).sort()).toEqual(["briefcase", "notes", "remind"]);
  expect(mainText(page)).toContain("3 apps");
  expect(page).toMatch(/<nav [^>]*aria-label="Categories"[^>]*>\s*<a href="\/categories\/productivity">Productivity<\/a>/);
});

test("a search with no results says what to try", async ({ request }) => {
  const page = await html(request, "/?q=zzzzqqqq");
  expect(resultIds(page)).toEqual([]);
  const text = mainText(page);
  expect(text).toContain("0 apps");
  expect(text).toContain("No apps match that. Try a shorter name or another spelling, or browse every app");
  expect(page).toContain('<a href="/apps">browse every app</a>');
});

test("an empty search is the home page", async ({ request }) => {
  const text = mainText(await html(request, "/?q="));
  expect(text).toContain("Core Apps to Try");
  expect(text).not.toContain("Apps matching");
});
