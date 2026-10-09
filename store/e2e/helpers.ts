/**
 * Shared helpers for the store's e2e suite. The fixture catalog and accounts come from scripts/seed-fixture.mjs:
 * public apps (briefcase, dm, ring, remind, notes, orbit, ledgerly, silicon-accounts, silicon-apps), one private app
 * shared with c:mira (team-vault), one draft (draft-thing), and fixture sessions for c:mira, si:nova and c:ada.
 */
import { expect, type APIRequestContext, type BrowserContext } from "@playwright/test";

export const SESSIONS = { mira: "e2e-mira-session", nova: "e2e-nova-session", ada: "e2e-ada-session" } as const;

/** Signs a browser context in as a fixture account (APPS_DEV_AUTH=1 only), as the API's callback would. */
export async function signIn(context: BrowserContext, baseURL: string, who: keyof typeof SESSIONS): Promise<void> {
  await context.addCookies([{ name: "apps_session", value: SESSIONS[who], url: baseURL, httpOnly: true, sameSite: "Lax" }]);
}

/** The page as curl sees it: server-rendered HTML, no script run. */
export async function html(request: APIRequestContext, path: string, headers: Record<string, string> = {}): Promise<string> {
  const response = await request.get(path, { headers: { Accept: "text/html", ...headers } });
  expect(response.status(), `${path} status`).toBe(200);
  expect(response.headers()["content-type"]).toContain("text/html");
  return response.text();
}

/** Visible text of <main>, tags stripped (what a reader or crawler gets without script). */
export function mainText(page: string): string {
  const main = /<main[\s\S]*?<\/main>/.exec(page)?.[0] ?? page;
  return decode(main.replace(/<script[\s\S]*?<\/script>/g, " ").replace(/<style[\s\S]*?<\/style>/g, " ").replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ");
}

function decode(text: string): string {
  return text
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#x27;|&#39;/g, "'");
}

/** Every <meta> name or property with its content. */
export function metaOf(page: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const tag of page.match(/<meta\s[^>]*>/g) ?? []) {
    const key = /(?:name|property)="([^"]+)"/.exec(tag)?.[1];
    const content = /content="([^"]*)"/.exec(tag)?.[1];
    if (key && content !== undefined && !(key in out)) out[key] = decode(content);
  }
  const title = /<title>([^<]*)<\/title>/.exec(page)?.[1];
  if (title) out.title = decode(title);
  const canonical = /<link rel="canonical" href="([^"]+)"/.exec(page)?.[1];
  if (canonical) out.canonical = canonical;
  return out;
}

type Node = Record<string, unknown>;

/** Every JSON-LD node on the page; each script must parse as JSON. */
export function jsonLd(page: string): Node[] {
  const nodes: Node[] = [];
  for (const match of page.matchAll(/<script type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/g)) {
    const data = JSON.parse(match[1]) as Node;
    expect(data["@context"]).toBe("https://schema.org");
    nodes.push(...((data["@graph"] as Node[]) ?? [data]));
  }
  return nodes;
}

export const typesOf = (nodes: Node[]) => nodes.map(node => String(node["@type"]));

/** The checks every public page passes: semantics, one h1, the full meta set and the site-wide JSON-LD. */
export function expectPageBasics(page: string, { canonical, indexed = true }: { canonical?: string; indexed?: boolean } = {}): Record<string, string> {
  expect(page).toMatch(/<html lang="en"/);
  expect(page.match(/<header[\s>]/g)?.length ?? 0).toBeGreaterThanOrEqual(1);
  expect(page).toMatch(/<nav [^>]*aria-label="Main"/);
  expect(page.match(/<main[\s>]/g)).toHaveLength(1);
  expect(page).toMatch(/<footer[\s>]/);
  expect(page.match(/<h1[\s>]/g)).toHaveLength(1);
  const meta = metaOf(page);
  for (const key of ["title", "description", "og:title", "og:description", "og:image", "og:url", "og:type", "og:site_name", "twitter:card"]) {
    expect(meta[key], `meta ${key}`).toBeTruthy();
  }
  expect(meta["twitter:card"]).toBe("summary_large_image");
  expect(meta.description.length).toBeLessThanOrEqual(200);
  if (canonical) {
    expect(meta.canonical).toBe(canonical);
    expect(meta["og:url"]).toBe(canonical);
  }
  expect(meta.robots ?? "index, follow").toContain(indexed ? "index" : "noindex");
  if (indexed) expect(meta.robots ?? "index").not.toContain("noindex");
  const types = typesOf(jsonLd(page));
  expect(types).toContain("Organization");
  expect(types).toContain("WebSite");
  // Never an em dash or en dash in what we write.
  expect(mainText(page)).not.toMatch(/[\u2013\u2014]/);
  return meta;
}
