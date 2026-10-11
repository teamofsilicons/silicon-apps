/**
 * The search page's additional filters, as plain data both the server (lib/catalog.ts, the no-script form) and the
 * filter toolbar island (components/store/search-toolbar.tsx) read: every field, its URL parameter, its values and
 * the words shown for them. Imports nothing server-only.
 */
import { TARGETS, targetLabel } from "./format";

/** How the API orders results: its own relevance, or one of its sorts. */
export const SORTS = ["relevance", "installs", "rating", "newest", "updated", "name"] as const;
export type Sort = (typeof SORTS)[number];

/** A window of time back from now, for when an app was added or last released. */
export const PERIODS = { week: 7, month: 31, year: 366 } as const;
export type Period = keyof typeof PERIODS;

export const MIN_INSTALLS = [10, 100, 1000] as const;
/** Minimum ratings, in half stars from one to five (the rating picker sets them by pointing at a star). */
export const MIN_RATINGS = [1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5] as const;

export const SORT_LABELS: Record<Sort, string> = {
  relevance: "Best match",
  installs: "Most installed",
  rating: "Highest rated",
  newest: "Newest",
  updated: "Recently updated",
  name: "Name",
};
export const PERIOD_LABELS: Record<Period, string> = { week: "Past week", month: "Past month", year: "Past year" };
export const installsLabel = (n: number) => `${n.toLocaleString("en-US")} or more`;
export const ratingLabel = (n: number) => (n >= 5 ? "5.0 stars only" : `${n.toFixed(1)} stars or more`);
export const VISIBILITY_LABELS = { public: "Public", private: "Private" } as const;

/** One additional filter: its URL parameter, its label and the values it offers (the empty value is "any"). */
export type FilterSpec = { param: string; label: string; options: Array<{ value: string; label: string }> };

export const FILTERS: FilterSpec[] = [
  { param: "target", label: "Platform", options: TARGETS.map(target => ({ value: target, label: targetLabel(target) })) },
  { param: "sort", label: "Sort by", options: SORTS.filter(sort => sort !== "relevance").map(sort => ({ value: sort, label: SORT_LABELS[sort] })) },
  { param: "added", label: "Added", options: Object.entries(PERIOD_LABELS).map(([value, label]) => ({ value, label })) },
  { param: "released", label: "Last release", options: Object.entries(PERIOD_LABELS).map(([value, label]) => ({ value, label })) },
  { param: "installs", label: "Installs", options: MIN_INSTALLS.map(n => ({ value: String(n), label: installsLabel(n) })) },
  { param: "rating", label: "Rating", options: MIN_RATINGS.map(n => ({ value: String(n), label: ratingLabel(n) })) },
  { param: "visibility", label: "Visibility", options: Object.entries(VISIBILITY_LABELS).map(([value, label]) => ({ value, label })) },
];
