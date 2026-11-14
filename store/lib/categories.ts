/**
 * Categories (Anil, 2026-11-14; first grouped by task, then given a store's usual names): the store's apps grouped so
 * browsing means something. Each app is in one category, in the order listed here; a category lists only the apps the visitor can
 * see, most installed first, and one with none is left out. An app in no category goes in "More apps" at the end, so
 * a new app is never lost. The home page shows the categories as tiles; /apps uses them as its chapters.
 * A first draft of the groups, by app id; change them here.
 */
import type { App } from "./types";

export type Category = { id: string; title: string; line: string; apps: string[] };

export const CATEGORIES: Category[] = [
  { id: "essentials", title: "Essentials", line: "Your account, and the store every app comes from.", apps: ["silicon-accounts", "silicon-apps"] },
  { id: "communication", title: "Communication", line: "Messages, calls, notifications and contacts.", apps: ["dm", "ring", "ting", "peek", "silicon-contacts"] },
  { id: "productivity", title: "Productivity", line: "Documents, slides, spreadsheets, files and notes.", apps: ["write", "present", "calc", "briefcase", "silicon-writings", "diary"] },
  { id: "business", title: "Business", line: "Tasks, reminders and shared workspaces.", apps: ["commit", "remind", "silicon-organizations"] },
  { id: "ai-agents", title: "AI and Agents", line: "Runtimes, browsers, devices, voices and tools for your Silicons.", apps: ["silicon", "browser", "extend", "omni", "mcport", "waveform"] },
  { id: "developer-tools", title: "Developer Tools", line: "Backends, webhooks, realtime streams and observability.", apps: ["silicon-cells", "hook", "silicon-stream", "spacestation", "starter"] },
  { id: "media", title: "Media", line: "Photos, music and the devices around you.", apps: ["lucent", "spotify", "silicon-control"] },
];

export type FilledCategory = Omit<Category, "apps"> & { apps: App[] };

/** The categories with the visitor's apps in them (most installed first), then "More apps" for the rest. */
export function categorize(apps: App[]): FilledCategory[] {
  const byId = new Map(apps.map(app => [app.app_id, app]));
  const placed = new Set<string>();
  const ranked = (list: App[]) => [...list].sort((a, b) => b.installs - a.installs);
  const filled = CATEGORIES.map(category => {
    const members = category.apps.flatMap(id => byId.get(id) ?? []);
    members.forEach(app => placed.add(app.app_id));
    return { ...category, apps: ranked(members) };
  }).filter(category => category.apps.length);
  const rest = apps.filter(app => !placed.has(app.app_id));
  return rest.length ? [...filled, { id: "more", title: "More Apps", line: "Everything else in the store.", apps: ranked(rest) }] : filled;
}

/** How many apps a category shows as a chapter on All Apps; "See all" opens the rest on its own page. */
export const CHAPTER_LIMIT = 6;

/** A category by its id, with the visitor's apps in it; null for an unknown id. */
export function findCategory(apps: App[], id: string): FilledCategory | null {
  return categorize(apps).find(category => category.id === id) ?? null;
}

export const CATEGORY_SORTS = [
  { value: "popular", label: "Most popular" },
  { value: "new", label: "Newest" },
  { value: "rated", label: "Top rated" },
] as const;
export type CategorySort = (typeof CATEGORY_SORTS)[number]["value"];

export const CATEGORY_SYSTEMS = [
  { value: "", label: "All platforms" },
  { value: "macos", label: "macOS" },
  { value: "linux", label: "Linux" },
  { value: "windows", label: "Windows" },
] as const;

/** Top rated counts only apps with at least this many reviews; the rest follow, most installed first. */
export const MIN_REVIEWS_FOR_RATED = 3;

const latest = (app: App) => [app.latest_production?.created_at, app.latest_development?.created_at].filter((date): date is string => Boolean(date)).sort().at(-1) ?? app.created_at ?? "";

/** A category's apps for its page: on one system (macos, linux, windows; empty for all), in the chosen order. */
export function arrange(apps: App[], sort: CategorySort, system: string): App[] {
  const onSystem = system ? apps.filter(app => app.targets.some(target => target.startsWith(`${system}-`))) : apps;
  const byInstalls = (a: App, b: App) => b.installs - a.installs;
  if (sort === "new") return [...onSystem].sort((a, b) => latest(b).localeCompare(latest(a)) || byInstalls(a, b));
  if (sort === "rated") {
    const rated = (app: App) => app.rating !== null && app.review_count >= MIN_REVIEWS_FOR_RATED;
    return [...onSystem].sort((a, b) => Number(rated(b)) - Number(rated(a)) || (rated(a) && rated(b) ? (b.rating ?? 0) - (a.rating ?? 0) : 0) || byInstalls(a, b));
  }
  return [...onSystem].sort(byInstalls);
}

/** The category an app is filed under, by its id (none for an app in no category). */
export function categoryOf(appId: string): Category | null {
  return CATEGORIES.find(category => category.apps.includes(appId)) ?? null;
}
