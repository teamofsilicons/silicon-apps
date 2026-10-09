/** An app's page: everything its authors set up, its releases and signatures, the install command and its copy button. */
import { expect, test } from "@playwright/test";
import { expectPageBasics, html, jsonLd, mainText, metaOf } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";

test("the app page is server-rendered with everything about the app", async ({ request }) => {
  const page = await html(request, "/apps/briefcase");
  const meta = expectPageBasics(page, { canonical: `${ORIGIN}/apps/briefcase` });
  expect(meta.title).toBe("Briefcase · Silicon Apps");
  expect(meta["og:image"]).toBe(`${ORIGIN}/apps/briefcase/og.png`);
  expect(page).toMatch(/<article[^>]*aria-labelledby="app-name"/);
  const text = mainText(page);
  for (const expected of [
    "Briefcase",
    "Briefcase keeps your files in one place",
    "$ silicon-apps install briefcase",
    "Shubham",
    "Head of Growth",
    "4.5",
    "1,840",
    "3.4.2",
    "3.5.0",
    "Signed by Silicon Apps and by c:shubham",
    "Signed every package",
    "Withdrawn",
    "3.4.3",
    "Sync could skip files whose names contain a space",
    "Apple silicon",
    "ARMv7",
    "Website",
    "Changelog",
    "Ratings and reviews",
    "Syncing a folder for my Silicon took one command.",
  ]) expect(text, expected).toContain(expected);
  // Logo, banner and every picture carry alt text.
  for (const img of page.match(/<main[\s\S]*<\/main>/)![0].match(/<img [^>]*>/g) ?? []) expect(img, img).toMatch(/alt="[^"]*"/);
  expect(page).toContain('alt="A terminal running briefcase sync ./reports, which uploads 14 files and reports the folder is in sync."');
  expect(page).toContain('alt="Briefcase logo"');

  const nodes = jsonLd(page);
  const app = nodes.find(node => node["@type"] === "SoftwareApplication") as Record<string, unknown> & { aggregateRating: { ratingValue: number; reviewCount: number }; offers: { price: string } };
  expect(app.name).toBe("Briefcase");
  expect(app.identifier).toBe("briefcase");
  expect(app.softwareVersion).toBe("3.4.2");
  expect(app.operatingSystem).toBe("macOS, Linux, Windows");
  expect(app.aggregateRating.ratingValue).toBe(4.5);
  expect(app.aggregateRating.reviewCount).toBe(4);
  expect(app.offers.price).toBe("0");
  const crumbs = nodes.find(node => node["@type"] === "BreadcrumbList") as { itemListElement: Array<{ name: string; item: string }> };
  expect(crumbs.itemListElement.map(item => item.name)).toEqual(["Home", "All apps", "Briefcase"]);
  expect(crumbs.itemListElement[2].item).toBe(`${ORIGIN}/apps/briefcase`);
});

test("an app with only development releases installs from the development channel", async ({ request }) => {
  const text = mainText(await html(request, "/apps/orbit"));
  expect(text).toContain("$ silicon-apps install 'orbit>dev'");
  expect(text).toContain("This app has development releases only so far");
  expect(text).toContain("Not signed yet");
  expect(text).not.toContain("Signed by Silicon Apps");
});

test("the copy button copies the install command", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/apps/briefcase");
  const copy = page.getByRole("button", { name: "Copy the install command" });
  await expect(copy).toBeVisible();
  await copy.click();
  await expect(page.getByRole("button", { name: "Copied" })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("silicon-apps install briefcase");
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("the command stays selectable text and the copy button is hidden", async ({ page }) => {
    await page.goto("/apps/briefcase");
    await expect(page.locator("#install code").first()).toHaveText("$ silicon-apps install briefcase");
    await expect(page.getByRole("button", { name: "Copy the install command" })).toBeHidden();
    // The carousel scrolls by itself; its pictures are there without script.
    await expect(page.getByRole("img", { name: /briefcase ls --json/ })).toBeAttached();
  });
});

test("private apps and drafts answer 404 the same way to anyone they are not shared with", async ({ request }) => {
  for (const path of ["/apps/team-vault", "/apps/draft-thing", "/apps/no-such-app"]) {
    const response = await request.get(path);
    expect(response.status(), path).toBe(404);
    const page = await response.text();
    expect(mainText(page)).toContain("We could not find that app");
    expect(metaOf(page).robots).toContain("noindex");
  }
});

test("each app has its own Open Graph image", async ({ request }) => {
  const image = await request.get("/apps/briefcase/og.png");
  expect(image.status()).toBe(200);
  expect(image.headers()["content-type"]).toBe("image/png");
  expect((await image.body()).length).toBeGreaterThan(10_000);
  const hidden = await request.get("/apps/team-vault/og.png", { maxRedirects: 0 });
  expect(hidden.status()).toBe(307);
  expect(hidden.headers().location).toBe("/og.png");
});
