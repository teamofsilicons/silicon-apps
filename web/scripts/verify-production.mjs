import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "@playwright/test";

const origins = process.argv.slice(2);
if (!origins.length)
  throw new Error(
    "Pass one or more deployed origins, for example https://apps.teamofsilicons.com",
  );
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  for (const value of origins) {
    const origin = new URL(value).origin;
    const docs = await fetch(`${origin}/docs`, { redirect: "manual" });
    assert.equal(docs.status, 308);
    assert.equal(
      docs.headers.get("location"),
      "https://developers.teamofsilicons.com/docs/apps",
    );
    const docsPage = await fetch(
      `${origin}/docs/reference/manifest?source=legacy`,
      { redirect: "manual" },
    );
    assert.equal(docsPage.status, 308);
    assert.equal(
      docsPage.headers.get("location"),
      "https://developers.teamofsilicons.com/docs/apps/reference/manifest?source=legacy",
    );
    const document = await fetch(`${origin}/store`);
    assert.equal(document.status, 200);
    assert.equal(document.headers.get("cache-control"), "no-cache");
    assert.ok(
      document.headers
        .get("content-security-policy")
        ?.includes("connect-src 'self'"),
    );
    const html = await document.text();
    const asset = html.match(/src="(\/assets\/[^\"]+\.js)"/)?.[1];
    assert.ok(asset, "Expected a compiled, hashed JavaScript asset");
    const javascript = await fetch(origin + asset);
    assert.equal(javascript.status, 200);
    assert.match(javascript.headers.get("cache-control"), /immutable/);
    assert.equal(
      (await fetch(`${origin}/assets/does-not-exist.js`)).status,
      404,
    );
    for (const name of ["install.sh", "install.ps1"]) {
      const response = await fetch(`${origin}/${name}`);
      assert.equal(response.status, 200);
      assert.match(response.headers.get("content-type"), /^text\/plain/);
      assert.equal(
        await response.text(),
        await readFile(
          new URL(`../../scripts/${name}`, import.meta.url),
          "utf8",
        ),
      );
    }
    const health = await fetch(`${origin}/health`);
    assert.equal(health.status, 200);
    assert.equal(health.headers.get("cache-control"), "no-store");
    const session = await fetch(`${origin}/v1/session`);
    assert.equal(session.status, 200);
    assert.equal((await session.json()).authenticated, false);
    const catalog = await fetch(`${origin}/v1/apps?limit=1`);
    assert.equal(catalog.status, 200);
    assert.ok(Array.isArray((await catalog.json()).items));
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => {
      localStorage.setItem("apps.telemetry", "false");
      window.__policyErrors = [];
      document.addEventListener("securitypolicyviolation", (event) =>
        window.__policyErrors.push(
          `${event.violatedDirective}:${event.blockedURI}`,
        ),
      );
    });
    for (const [path, heading] of [
      ["/store", "Explore apps"],
      ["/settings", "Settings"],
    ]) {
      await page.goto(origin + path);
      await page.getByRole("heading", { name: heading, exact: true }).waitFor();
      assert.deepEqual(await page.evaluate(() => window.__policyErrors), []);
      if (path === "/store") {
        await page
          .getByRole("button", { name: "Private", exact: true })
          .click();
        await page
          .getByRole("heading", { name: "Log in to see private apps" })
          .waitFor();
      }
    }
    await page.goto(origin + "/");
    await page.waitForURL("**/store");
    assert.equal(
      await page
        .getByRole("link", { name: "Developers", exact: true })
        .getAttribute("href"),
      "https://developers.teamofsilicons.com/",
    );
    assert.equal(
      await page
        .getByRole("link", { name: "Docs", exact: true })
        .getAttribute("href"),
      "https://developers.teamofsilicons.com/docs/apps",
    );
    const management = await fetch(origin + "/developer/apps/apps?step=3", {
      redirect: "manual",
    });
    assert.equal(management.status, 308);
    assert.equal(
      management.headers.get("location"),
      "https://developers.teamofsilicons.com/apps/apps/publishing?step=3",
    );
    assert.deepEqual(errors, []);
    await page.close();
    console.log(
      `${origin}: store routes, shared docs redirects, external management links, CSP, assets/cache, installer bytes and API proxy verified.`,
    );
  }
} finally {
  await browser.close();
}
