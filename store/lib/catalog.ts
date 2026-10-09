/**
 * Catalog reads built on the API's search (`GET /v1/apps`): it already ranks exact ids and names first, then prefixes,
 * substrings and typo matches, and only returns apps the visitor may see. Tag and platform filters are applied here,
 * on top of that ranking, because the API has no parameters for them.
 */
import { api } from "./api";
import { TARGETS } from "./format";
import type { App, AppList } from "./types";

export const PAGE_SIZE = 24;
const MAX_SCAN = 1000;

export type Visibility = "all" | "public" | "private";

export type SearchInput = {
  q: string;
  tag: string;
  target: string;
  visibility: Visibility;
  page: number;
};

export type SearchResult = { items: App[]; total: number; page: number; pages: number };

function query(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) if (value !== undefined && value !== "") search.set(key, String(value));
  const text = search.toString();
  return text ? `?${text}` : "";
}

export async function listPage(q: string, visibility: Visibility, limit: number, offset: number, anonymous = false): Promise<AppList> {
  return api<AppList>(
    `/v1/apps${query({ q: q || undefined, visibility: visibility === "all" ? undefined : visibility, limit, offset })}`,
    { anonymous },
  );
}

/** Every app the visitor can see for this query, in the API's order (bounded). */
export async function listAll(q = "", visibility: Visibility = "all", anonymous = false): Promise<App[]> {
  const items: App[] = [];
  for (let offset = 0; offset < MAX_SCAN; offset += 100) {
    const page = await listPage(q, visibility, 100, offset, anonymous);
    items.push(...page.items);
    if (items.length >= page.total || page.items.length === 0) break;
  }
  return items;
}

export function parseSearch(params: Record<string, string | string[] | undefined>): SearchInput {
  const one = (key: string) => {
    const value = params[key];
    return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
  };
  const visibility = one("visibility");
  const target = one("target");
  const page = Math.floor(Number(one("page")) || 1);
  return {
    q: one("q").slice(0, 200),
    tag: one("tag").toLowerCase().slice(0, 60),
    target: (TARGETS as readonly string[]).includes(target) ? target : "",
    visibility: visibility === "public" || visibility === "private" ? visibility : "all",
    page: Math.min(Math.max(page, 1), 1000),
  };
}

export async function search(input: SearchInput): Promise<SearchResult> {
  const offset = (input.page - 1) * PAGE_SIZE;
  if (!input.tag && !input.target) {
    const result = await listPage(input.q, input.visibility, PAGE_SIZE, offset);
    return { items: result.items, total: result.total, page: input.page, pages: Math.max(1, Math.ceil(result.total / PAGE_SIZE)) };
  }
  const matching = (await listAll(input.q, input.visibility)).filter(
    (app) =>
      (!input.tag || app.tags.some((tag) => tag.toLowerCase() === input.tag)) &&
      (!input.target || app.targets.includes(input.target)),
  );
  return {
    items: matching.slice(offset, offset + PAGE_SIZE),
    total: matching.length,
    page: input.page,
    pages: Math.max(1, Math.ceil(matching.length / PAGE_SIZE)),
  };
}

export function tagCounts(apps: App[]): { tag: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const app of apps) for (const tag of new Set(app.tags.map((t) => t.toLowerCase().trim()).filter(Boolean))) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  return [...counts.entries()].map(([tag, count]) => ({ tag, count })).sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag));
}

/** A Bayesian average, so one five-star review does not outrank fifty four-star ones. */
function score(app: App): number {
  const prior = 3.5;
  const weight = 3;
  const rating = app.rating ?? prior;
  const rated = (rating * app.review_count + prior * weight) / (app.review_count + weight);
  return rated * 10 + Math.log10(app.installs + 1) * 4 + (app.banner ? 1 : 0) + (app.logo ? 1 : 0);
}

/**
 * Home page sections. Each app appears once: featured first (best rated and most installed), then popular (installs)
 * and new (newest first) share what is left, up to six each, so a small catalog still shows all three without
 * repeating itself. Empty sections are left out.
 */
export function homeSections(apps: App[]): { featured: App[]; popular: App[]; fresh: App[] } {
  const seen = new Set<string>();
  const take = (list: App[], count: number) => {
    const picked = list.filter((app) => !seen.has(app.app_id)).slice(0, count);
    picked.forEach((app) => seen.add(app.app_id));
    return picked;
  };
  const featured = take([...apps].sort((a, b) => score(b) - score(a) || a.name.localeCompare(b.name)), 3);
  const rest = apps.length - featured.length;
  const popular = take([...apps].sort((a, b) => b.installs - a.installs || a.name.localeCompare(b.name)), Math.min(6, Math.ceil(rest / 2)));
  const fresh = take([...apps].sort((a, b) => b.created_at.localeCompare(a.created_at) || a.name.localeCompare(b.name)), 6);
  return { featured, popular, fresh };
}
