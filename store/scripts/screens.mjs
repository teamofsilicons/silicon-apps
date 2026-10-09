#!/usr/bin/env node
/**
 * Full-page screenshots of the store for a visual check: home, search and an app page, light and dark, at 320 and
 * 1440 pixels wide, into .screens/ (ignored by git). Run it against a running store (pnpm dev or the standalone
 * server) backed by the fixture API (scripts/e2e-api.mjs):
 *
 *   node scripts/screens.mjs [--base http://127.0.0.1:8710] [--app briefcase] [--signed-in]
 *
 * --signed-in adds the fixture session cookie of c:mira (APPS_DEV_AUTH=1 only), to see the review form.
 */
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";

const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const base = option("--base", "http://127.0.0.1:8710").replace(/\/+$/, "");
const appId = option("--app", "briefcase");
const signedIn = args.includes("--signed-in");
const out = resolve(".screens");
mkdirSync(out, { recursive: true });

const pages = [
  ["home", "/"],
  ["search", "/search?q=note"],
  ["tag", "/search?tag=productivity"],
  ["app", `/apps/${appId}`],
];

const browser = await chromium.launch();
const saved = [];
for (const scheme of ["light", "dark"]) {
  for (const width of [320, 1440]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, colorScheme: scheme, deviceScaleFactor: width < 600 ? 2 : 1, reducedMotion: "reduce" });
    if (signedIn) await context.addCookies([{ name: "apps_session", value: "e2e-mira-session", url: base }]);
    const page = await context.newPage();
    for (const [name, path] of pages) {
      await page.goto(`${base}${path}`, { waitUntil: "networkidle" });
      await page.evaluate(() => document.fonts.ready);
      const file = join(out, `${name}-${scheme}-${width}.png`);
      await page.screenshot({ path: file, fullPage: true });
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      saved.push(`${file}${overflow > 0 ? ` (scrolls sideways by ${overflow}px)` : ""}`);
    }
    await context.close();
  }
}
await browser.close();
console.log(saved.join("\n"));
