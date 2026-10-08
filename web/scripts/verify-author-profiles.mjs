import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '@playwright/test';

const origin = new URL(process.argv[2] || 'https://apps.teamofsilicons.com').origin;
const directory = new URL('../../.dev/author-profiles-live/', import.meta.url);
await mkdir(directory, { recursive: true });
const appResponse = await fetch(`${origin}/v1/apps/silicon-apps`);
assert.equal(appResponse.status, 200);
const app = await appResponse.json();
assert.ok(app.authors.length);
const profiles = [];
for (const author of app.authors) {
  const response = await fetch(`${origin}/v1/authors/${encodeURIComponent(author.uuid)}?limit=100`);
  assert.equal(response.status, 200);
  const profile = await response.json();
  assert.equal(profile.uuid, author.uuid);
  assert.ok(profile.items.some(item => item.app_id === app.app_id));
  assert.ok(profile.items.every(item => item.published && item.visibility === 'public' && item.authors.some(a => a.uuid === author.uuid)));
  profiles.push({ uuid: profile.uuid, id: profile.id, total: profile.total, app_ids: profile.items.map(item => item.app_id) });
}
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [];
try {
  for (const width of [390, 1440]) {
    const context = await browser.newContext({ viewport: { width, height: 1000 }, colorScheme: 'light' });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      localStorage.setItem('apps.telemetry', 'false');
      window.__policyErrors = [];
      document.addEventListener('securitypolicyviolation', event => window.__policyErrors.push(`${event.violatedDirective}:${event.blockedURI}`));
    });
    for (const theme of ['light', 'dark']) {
      await page.goto(`${origin}/store/silicon-apps`);
      await page.getByRole('heading', { name: app.name, exact: true }).waitFor();
      if (theme === 'dark') {
        await page.getByRole('button', { name: 'Switch to dark mode' }).click();
        await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark' && !document.documentElement.hasAttribute('data-transition'));
      }
      assert.equal(await page.locator('html').getAttribute('data-theme'), theme);
      const byline = page.getByRole('list', { name: 'App authors' });
      assert.equal(await byline.getByRole('link').count(), app.authors.length);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      await page.screenshot({ path: new URL(`app-${theme}-${width}.png`, directory).pathname, fullPage: true });
      await byline.getByRole('link').first().click();
      await page.getByRole('heading', { name: 'Published apps', exact: true }).waitFor();
      assert.equal(new URL(page.url()).pathname, '/authors/' + encodeURIComponent(app.authors[0].uuid));
      assert.equal(await page.locator('.app-card').count(), Math.min(24, profiles[0].total));
      await page.reload();
      await page.getByRole('heading', { name: 'Published apps', exact: true }).waitFor();
      assert.equal(await page.locator('html').getAttribute('data-theme'), theme);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      assert.deepEqual(await page.evaluate(() => window.__policyErrors), []);
      await page.screenshot({ path: new URL(`profile-${theme}-${width}.png`, directory).pathname, fullPage: true });
    }
    await context.close();
  }
  assert.deepEqual(errors, []);
  const report = { origin, profiles, widths: [390, 1440], themes: ['light', 'dark'], screenshots: 8, console_errors: errors, checked_at: new Date().toISOString() };
  await writeFile(new URL('verification.json', directory), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
} finally { await browser.close(); }
