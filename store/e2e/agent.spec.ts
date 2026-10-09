/**
 * The agent files (robots.txt, sitemap.xml, llms.txt, llms-full.txt, security.txt, manifest), the API paths, and that
 * pages register no tools in the browser (agents call the MCP server at /mcp, e2e/mcp.spec.ts).
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

const ORIGIN = "https://apps.teamofsilicons.com";

test("robots.txt welcomes crawlers and agents and names the sitemap", async ({ request }) => {
  const response = await request.get("/robots.txt");
  expect(response.headers()["content-type"]).toBe("text/plain; charset=utf-8");
  const body = await response.text();
  for (const line of ["User-agent: *", "Allow: /", "User-agent: GPTBot", "User-agent: ClaudeBot", "User-agent: PerplexityBot", "User-agent: Google-Extended", "Disallow: /settings", "Disallow: /sign-in", "Disallow: /api/", `Sitemap: ${ORIGIN}/sitemap.xml`]) {
    expect(body).toContain(line);
  }
  expect(body).not.toMatch(/^Disallow: \/$/m);
});

test("sitemap.xml lists the home page, every public app and their authors, with lastmod", async ({ request }) => {
  const response = await request.get("/sitemap.xml");
  expect(response.headers()["content-type"]).toBe("application/xml; charset=utf-8");
  const body = await response.text();
  expect(body.startsWith('<?xml version="1.0" encoding="UTF-8"?>')).toBe(true);
  const locations = [...body.matchAll(/<loc>([^<]+)<\/loc>/g)].map(match => match[1]);
  for (const path of ["/", "/search", "/apps/briefcase", "/apps/dm", "/apps/orbit", "/authors/shubham", "/authors/head_of_growth", "/llms.txt"]) {
    expect(locations).toContain(path === "/" ? `${ORIGIN}/` : `${ORIGIN}${path}`);
  }
  expect(body).not.toContain("team-vault");
  expect(body).not.toContain("draft-thing");
  expect(body.match(/<url>/g)?.length).toBe(body.match(/<lastmod>\d{4}-\d\d-\d\dT[^<]+<\/lastmod>/g)?.length);
});

test("llms.txt and llms-full.txt are store/llms/llms.md exactly as written", async ({ request }) => {
  const source = readFileSync(join(__dirname, "..", "llms", "llms.md"), "utf8");
  for (const path of ["/llms.txt", "/llms-full.txt"]) {
    const response = await request.get(path);
    expect(response.status(), path).toBe(200);
    expect(response.headers()["content-type"], path).toBe("text/plain; charset=utf-8");
    expect(await response.text(), path).toBe(source);
    expect(response.headers().etag, path).toBeTruthy();
  }
});

test("security.txt, the manifest and the icons", async ({ request }) => {
  const security = await (await request.get("/.well-known/security.txt")).text();
  expect(security).toContain("Contact: mailto:");
  expect(security).toMatch(/^Expires: \d{4}-/m);
  const manifest = await request.get("/manifest.webmanifest");
  expect(manifest.headers()["content-type"]).toContain("application/manifest+json");
  const parsed = await manifest.json();
  expect(parsed.name).toBe("Silicon Apps");
  for (const icon of parsed.icons as Array<{ src: string }>) expect((await request.get(icon.src)).status(), icon.src).toBe(200);
  for (const path of ["/favicon.ico", "/icon.svg", "/apple-touch-icon.png", "/og.png"]) expect((await request.get(path)).status(), path).toBe(200);
});

test("the API's own paths reach the Apps API (Caddy's job in production)", async ({ request }) => {
  for (const path of ["/health", "/openapi.json", "/.well-known/agent.json", "/.well-known/agent-card.json", "/.well-known/silicon-apps-keys.json", "/v1/apps?q=briefcase", "/v1/capabilities"]) {
    const response = await request.get(path);
    expect(response.status(), path).toBe(200);
    expect(response.headers()["content-type"], path).toContain("application/json");
  }
  const card = await (await request.get("/.well-known/agent.json")).json();
  expect(card.name).toBeTruthy();
  const list = await (await request.get("/v1/apps?q=briefcase")).json();
  expect(list.items[0].app_id).toBe("briefcase");
});

test("pages register no tools in the browser: no modelContext script, and nothing registers when it is offered", async ({ page, request }) => {
  for (const path of ["/", "/search", "/apps/briefcase"]) {
    const source = await (await request.get(path)).text();
    expect(source, path).not.toContain("modelContext");
    expect(source, path).not.toContain("registerTool");
    expect(source, path).not.toContain("WebMCP");
  }

  await page.addInitScript(() => {
    const calls: string[] = [];
    Object.defineProperty(window, "__calls", { value: calls });
    Object.defineProperty(navigator, "modelContext", {
      value: { registerTool: () => calls.push("registerTool"), provideContext: () => calls.push("provideContext") },
      configurable: true,
    });
  });
  await page.goto("/search");
  await page.waitForLoadState("networkidle");
  const calls = await page.evaluate(() => (window as unknown as { __calls: string[] }).__calls);
  expect(calls).toEqual([]);
});
