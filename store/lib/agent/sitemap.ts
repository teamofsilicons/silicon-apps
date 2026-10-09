/**
 * /sitemap.xml: every public page with its last change: the home page, all apps, every public app page, the page of
 * every author of a public app, the tag pages, and the two agent files. Private apps, Settings and typed searches are
 * not pages to index. Docs live on developers.teamofsilicons.com, in its own sitemap (a sitemap lists its own host).
 */
import "server-only";
import { LLMS_TXT_MODIFIED } from "@/lib/agent/generated/llms";
import { listAll, tagCounts } from "@/lib/catalog";
import { appPath, authorPath } from "@/lib/seo";
import { CANONICAL_ORIGIN } from "@/lib/site";

interface Entry {
  path: string;
  modified: string | null;
  priority: number;
}

const escapeXml = (value: string) => value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");

const latest = (dates: Array<string | null | undefined>): string | null => dates.filter((date): date is string => Boolean(date) && !Number.isNaN(Date.parse(date!))).sort().at(-1) ?? null;

/** Throws when the catalog cannot be read (the route answers 503 rather than an empty sitemap). */
export async function sitemapEntries(): Promise<Entry[]> {
  const apps = await listAll("", "public", true);
  const newest = latest(apps.map(app => app.updated_at));
  const entries: Entry[] = [
    { path: "/", modified: newest, priority: 1 },
    { path: "/search", modified: newest, priority: 0.8 },
  ];
  for (const app of apps) entries.push({ path: appPath(app.app_id), modified: app.updated_at, priority: 0.9 });
  const authors = new Map<string, string>();
  for (const app of apps) for (const author of app.authors) if ((authors.get(author.uuid) ?? "") < app.updated_at) authors.set(author.uuid, app.updated_at);
  for (const [uuid, modified] of authors) entries.push({ path: authorPath(uuid), modified, priority: 0.6 });
  for (const { tag } of tagCounts(apps)) {
    entries.push({ path: `/search?tag=${encodeURIComponent(tag)}`, modified: latest(apps.filter(app => app.tags.some(t => t.toLowerCase() === tag)).map(app => app.updated_at)), priority: 0.5 });
  }
  entries.push({ path: "/llms.txt", modified: LLMS_TXT_MODIFIED ?? newest, priority: 0.5 });
  entries.push({ path: "/llms-full.txt", modified: LLMS_TXT_MODIFIED ?? newest, priority: 0.5 });
  const seen = new Set<string>();
  return entries.filter(entry => (seen.has(entry.path) ? false : (seen.add(entry.path), true)));
}

export async function sitemapXml(): Promise<string> {
  const urls = (await sitemapEntries()).map(entry => {
    const location = entry.path === "/" ? `${CANONICAL_ORIGIN}/` : `${CANONICAL_ORIGIN}${entry.path}`;
    return [
      "  <url>",
      `    <loc>${escapeXml(location)}</loc>`,
      entry.modified ? `    <lastmod>${new Date(entry.modified).toISOString()}</lastmod>` : null,
      `    <priority>${entry.priority.toFixed(1)}</priority>`,
      "  </url>",
    ].filter(Boolean).join("\n");
  });
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join("\n")}\n</urlset>\n`;
}
