/**
 * Catalog reads built on the API's search (`GET /v1/apps`): it already ranks exact ids and names first, then prefixes,
 * substrings and typo matches, sorts on request, and only returns apps the visitor may see. Tags are matched through
 * the query text (the API searches ids, names, descriptions and tags); they are never a filter of their own. The
 * additional filters (platform, when it was added, its last release, installs and rating) are applied here, on top of
 * the API's order.
 */
import { api } from "./api";
import { TARGETS } from "./format";
import type { App, AppList } from "./types";

export const PAGE_SIZE = 24;
const MAX_SCAN = 1000;

export type Visibility = "all" | "public" | "private";

/** How the API orders results: its own relevance, or one of its sorts. */
export const SORTS = ["relevance", "installs", "rating", "newest", "updated", "name"] as const;
export type Sort = (typeof SORTS)[number];

/** A window of time back from now, for when an app was added or last released. */
export const PERIODS = { week: 7, month: 31, year: 366 } as const;
export type Period = keyof typeof PERIODS;

export const MIN_INSTALLS = [10, 100, 1000] as const;
export const MIN_RATINGS = [3, 4, 4.5] as const;

export type SearchInput = {
  q: string;
  target: string;
  visibility: Visibility;
  /** Added to the store within this window. */
  added: Period | "";
  /** Newest release within this window. */
  released: Period | "";
  minInstalls: number;
  minRating: number;
  sort: Sort;
  page: number;
};

export type SearchResult = { items: App[]; total: number; page: number; pages: number };

/** How many additional filters are set (the sort counts, the search text does not). */
export function activeFilters(input: SearchInput): number {
  return [input.target, input.visibility !== "all", input.added, input.released, input.minInstalls, input.minRating, input.sort !== "relevance"].filter(Boolean).length;
}

function query(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) if (value !== undefined && value !== "") search.set(key, String(value));
  const text = search.toString();
  return text ? `?${text}` : "";
}

export async function listPage(q: string, visibility: Visibility, limit: number, offset: number, anonymous = false, sort: Sort = "relevance"): Promise<AppList> {
  return api<AppList>(
    `/v1/apps${query({ q: q || undefined, visibility: visibility === "all" ? undefined : visibility, sort: sort === "relevance" ? undefined : sort, limit, offset })}`,
    { anonymous },
  );
}

/** Every app the visitor can see for this query, in the API's order (bounded). */
export async function listAll(q = "", visibility: Visibility = "all", anonymous = false, sort: Sort = "relevance"): Promise<App[]> {
  const items: App[] = [];
  for (let offset = 0; offset < MAX_SCAN; offset += 100) {
    const page = await listPage(q, visibility, 100, offset, anonymous, sort);
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
  const period = (key: string): Period | "" => (one(key) in PERIODS ? (one(key) as Period) : "");
  const installs = Number(one("installs"));
  const rating = Number(one("rating"));
  const sort = one("sort");
  return {
    q: one("q").slice(0, 200),
    target: (TARGETS as readonly string[]).includes(target) ? target : "",
    visibility: visibility === "public" || visibility === "private" ? visibility : "all",
    added: period("added"),
    released: period("released"),
    minInstalls: (MIN_INSTALLS as readonly number[]).includes(installs) ? installs : 0,
    minRating: (MIN_RATINGS as readonly number[]).includes(rating) ? rating : 0,
    sort: (SORTS as readonly string[]).includes(sort) ? (sort as Sort) : "relevance",
    page: Math.min(Math.max(page, 1), 1000),
  };
}

/** The newest release of an app (production or development), or its creation when it has none. */
function lastRelease(app: App): string {
  return [app.latest_production?.created_at, app.latest_development?.created_at].filter((date): date is string => Boolean(date)).sort().at(-1) ?? "";
}

function within(iso: string, period: Period | "", now: number): boolean {
  if (!period) return true;
  const time = Date.parse(iso);
  return Number.isFinite(time) && now - time <= PERIODS[period] * 86_400_000;
}

export async function search(input: SearchInput): Promise<SearchResult> {
  const offset = (input.page - 1) * PAGE_SIZE;
  const narrowing = input.target || input.added || input.released || input.minInstalls || input.minRating;
  if (!narrowing) {
    const result = await listPage(input.q, input.visibility, PAGE_SIZE, offset, false, input.sort);
    return { items: result.items, total: result.total, page: input.page, pages: Math.max(1, Math.ceil(result.total / PAGE_SIZE)) };
  }
  const now = Date.now();
  const matching = (await listAll(input.q, input.visibility, false, input.sort)).filter(
    (app) =>
      (!input.target || app.targets.includes(input.target)) &&
      within(app.created_at, input.added, now) &&
      within(lastRelease(app), input.released, now) &&
      app.installs >= input.minInstalls &&
      (!input.minRating || (app.review_count > 0 && (app.rating ?? 0) >= input.minRating)),
  );
  return {
    items: matching.slice(offset, offset + PAGE_SIZE),
    total: matching.length,
    page: input.page,
    pages: Math.max(1, Math.ceil(matching.length / PAGE_SIZE)),
  };
}
