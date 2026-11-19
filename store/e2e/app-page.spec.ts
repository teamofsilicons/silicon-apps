/**
 * An app's page (the Sidebar layout): everything its authors set up, its releases and signatures, the install command
 * and its copy button, compatibility with the reasons for missing builds, and who may see it.
 */
import { expect, test } from "@playwright/test";
import { AUTHORS, expectPageBasics, html, mainText, metaOf, signIn } from "./helpers";

const ORIGIN = "https://apps.teamofsilicons.com";
const RING_WINDOWS = "Ring places calls through the PulseAudio and Core Audio servers, and Windows has no audio routing service a command-line tool can use without a signed kernel driver. We will ship a Windows build as soon as that changes.";

test("the app page is server-rendered with everything about the app", async ({ request }) => {
  const page = await html(request, "/apps/briefcase");
  const meta = expectPageBasics(page, { canonical: `${ORIGIN}/apps/briefcase` });
  expect(meta.title).toBe("Briefcase · Silicon Apps");
  expect(meta.description).toContain("Briefcase keeps your files in one place");
  expect(page).toMatch(/<article[^>]*aria-labelledby="app-name"/);
  expect(page).toMatch(/<h1 id="app-name"[^>]*>Briefcase</);
  // The install command, the facts and the developer card sit in the sidebar beside the reading column.
  expect(page).toMatch(/<aside [^>]*aria-label="Get it"/);
  const text = mainText(page);
  for (const expected of [
    "Keeps your files in one place for you and your Silicons.",
    "Open Website",
    "$ silicon-apps install briefcase",
    "Installs the latest release for your system and keeps it up to date.",
    "Category Productivity",
    "Version 3.4.2",
    "Systems macOS Linux Windows",
    "Signed by Silicon Apps and developer",
    "Shubham",
    "c:shubham",
    "Head of Growth",
    "Changelog",
    "About",
    "Sync any folder with one command",
    "What’s new",
    "Version 3.4.2 with faster startup and clearer errors.",
    "Withdrawn releases",
    "3.4.3 · Production",
    "Sync could skip files whose names contain a space",
    "The development release, 3.5.0, newer than production",
    "$ silicon-apps install 'briefcase>dev'",
    "$ silicon-apps install 'briefcase@3.4.2'",
    "Compatibility",
    "Apple silicon",
    "ARMv7",
    "Ratings and reviews",
    "4.5 out of 5 · 4 ratings",
    "Syncing a folder for my Silicon took one command.",
    "More by these developers",
    "Remind",
  ]) expect(text, expected).toContain(expected);
  // Every processor is built, so nothing is marked missing and no reason is given.
  expect(text).not.toContain("Not available");
  expect(text).not.toMatch(/No [A-Za-z0-9 -]+ build\./);
  // Who signed it, in full, on the signature fact.
  expect(page).toContain('title="Signed by Silicon Apps and by c:shubham"');
  // The developers link to their own pages, by UUID.
  expect(page).toContain(`href="/authors/${AUTHORS.shubham}"`);
  expect(page).toContain(`href="/authors/${AUTHORS.growth}"`);
  // Every picture carries alt text: the screenshots their authors' words, the icon none (the name is beside it).
  for (const img of page.match(/<main[\s\S]*<\/main>/)![0].match(/<img [^>]*>/g) ?? []) expect(img, img).toMatch(/alt="[^"]*"/);
  expect(page).toContain('alt="A terminal running briefcase sync ./reports, which uploads 14 files and reports the folder is in sync."');
});

test("an app with only development releases installs from the development channel", async ({ request }) => {
  const text = mainText(await html(request, "/apps/orbit"));
  expect(text).toContain("$ silicon-apps install 'orbit>dev'");
  expect(text).not.toContain("$ silicon-apps install orbit ");
  expect(text).toContain("Only development releases so far, so this installs the latest one.");
  expect(text).toContain("The latest development release");
  expect(text).toContain("Signed Not signed yet");
  expect(text).not.toContain("Signed by");
  expect(text).not.toContain("Withdrawn releases");
});

test("the copy button copies the install command", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/apps/briefcase");
  const copy = page.getByRole("complementary", { name: "Get it" }).getByRole("button", { name: "Copy the command that installs Briefcase" });
  await expect(copy).toBeVisible();
  await copy.click();
  // Silicon UI's CopyButton keeps its name and says so in its own status line.
  await expect(page.getByRole("status").filter({ hasText: "Copy the command that installs Briefcase: Copied" })).toBeAttached();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("silicon-apps install briefcase");
});

test("new to the CLI, the note under the command links to how to install it", async ({ page }) => {
  await page.goto("/apps/briefcase");
  const get = page.getByRole("complementary", { name: "Get it" });
  await expect(get.getByText("New to it?")).toBeVisible();
  await expect(get.getByRole("link", { name: "Get the CLI" })).toHaveAttribute("href", "https://developers.teamofsilicons.com/docs/apps/start/install");
});

test.describe("with JavaScript off", () => {
  test.use({ javaScriptEnabled: false });

  test("the command stays selectable text and the copy buttons are hidden", async ({ page }) => {
    await page.goto("/apps/briefcase");
    const get = page.getByRole("complementary", { name: "Get it" });
    await expect(get.locator("code").first()).toHaveText("$ silicon-apps install briefcase");
    await expect(get.locator("code").first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Copy the command that installs Briefcase" })).toBeHidden();
    // The screenshots are a strip that scrolls by itself; its pictures are there without script.
    const shots = page.getByRole("list", { name: "Screenshots of Briefcase" });
    await expect(shots.getByRole("img")).toHaveCount(3);
    await expect(shots.getByRole("img", { name: /briefcase ls --json/ })).toBeAttached();
  });
});

test("private apps and drafts answer 404 the same way to anyone they are not shared with", async ({ request }) => {
  for (const path of ["/apps/team-vault", "/apps/draft-thing", "/apps/no-such-app"]) {
    const response = await request.get(path);
    expect(response.status(), path).toBe(404);
    const page = await response.text();
    expect(mainText(page), path).toContain("This page drifted off.");
    expect(page, path).not.toContain("Team Vault");
    expect(page, path).not.toContain("Draft Thing");
    expect(metaOf(page).robots, path).toContain("noindex");
  }
});

test("a private app shows to the accounts it is shared with, marked private, and to no one else", async ({ browser, baseURL }) => {
  const mira = await browser.newContext({ baseURL });
  await signIn(mira, baseURL!, "mira");
  const miraPage = await mira.newPage();
  const shared = await miraPage.goto("/apps/team-vault");
  expect(shared?.status()).toBe(200);
  const heading = miraPage.getByRole("heading", { level: 1 });
  await expect(heading).toContainText("Team Vault");
  await expect(heading.getByText("Private", { exact: true })).toBeVisible();
  // All Apps lists it for them too.
  await miraPage.goto("/apps");
  await expect(miraPage.locator("main").getByRole("link", { name: /Team Vault/ })).toBeVisible();
  await mira.close();

  const nova = await browser.newContext({ baseURL });
  await signIn(nova, baseURL!, "nova");
  const novaPage = await nova.newPage();
  const hidden = await novaPage.goto("/apps/team-vault");
  expect(hidden?.status()).toBe(404);
  await expect(novaPage.getByRole("heading", { level: 1 })).toHaveText("404 This page drifted off.");
  await novaPage.goto("/apps");
  await expect(novaPage.locator("main").getByRole("link", { name: /Team Vault/ })).toHaveCount(0);
  await nova.close();
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

test("a required platform the authors left out is marked missing, with their reason, without script", async ({ page, request }) => {
  // Server-rendered: the reason sits under the system's row in Compatibility.
  const text = mainText(await html(request, "/apps/ring"));
  expect(text).toContain("Systems macOS Linux Signed");
  expect(text).toContain(`No Windows x64 build. ${RING_WINDOWS}`);
  // Only Windows x64 is a required target with a reason; the other missing Windows builds are just marked.
  expect(text.match(/No [A-Za-z0-9 -]+ build\./g)).toEqual(["No Windows x64 build."]);

  await page.goto("/apps/ring");
  const compatibility = page.getByRole("region", { name: "Compatibility" });
  const windows = compatibility.getByRole("list", { name: "Windows processors" }).getByRole("listitem");
  await expect(windows).toHaveText(["Not available: x64", "Not available: ARM64", "Not available: x86 32-bit"]);
  const mac = compatibility.getByRole("list", { name: "macOS processors" }).getByRole("listitem");
  await expect(mac).toHaveText(["Apple silicon", "Intel"]);
  const linux = compatibility.getByRole("list", { name: "Linux processors" }).getByRole("listitem");
  await expect(linux).toHaveText(["x86-64", "ARM64", "x86 32-bit", "ARMv7"]);
  await expect(compatibility.getByText(`No Windows x64 build. ${RING_WINDOWS}`)).toBeVisible();
});
