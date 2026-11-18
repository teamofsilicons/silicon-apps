/**
 * An app's release history (GET /v1/apps/{app_id}/releases?channel=…; Anil, 2026-11-18, for What's new): production's,
 * else development's when it has no production release yet, newest first. The catalog's apps have 3 to 8 each, with
 * notes of 0 to 1,230 characters. Empty when the API doesn't answer.
 */
import { apiOrNull } from "@/lib/api";
import type { App, Release } from "@/lib/types";

const byNewest = (a: Release, b: Release) => b.created_at.localeCompare(a.created_at);

export async function releaseHistory(app: App): Promise<Release[]> {
  const channel = app.latest_production ? "production" : "development";
  const list = await apiOrNull<{ items: Release[] }>(`/v1/apps/${encodeURIComponent(app.app_id)}/releases?channel=${channel}`).catch(() => null);
  return [...(list?.items ?? [])].filter(release => !release.withdrawn).sort(byNewest);
}

/** How long ago `iso` was, in words: "today", "yesterday", "5 days ago", "3 weeks ago", "2 months ago", "a year ago". */
export function ago(iso: string, now = Date.now()): string {
  const days = Math.max(0, Math.floor((now - Date.parse(iso)) / 86_400_000));
  if (days === 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  if (days < 60) return `${Math.round(days / 7)} weeks ago`;
  if (days < 365) return `${Math.round(days / 30)} months ago`;
  const years = Math.round(days / 365);
  return years === 1 ? "a year ago" : `${years} years ago`;
}

/** Where `iso` falls between `from` and now, as a CSS percentage (0% to 100%), for a strip of releases over time. */
export function placeInTime(iso: string, from: string, now = Date.now()): string {
  const start = Date.parse(from);
  const span = Math.max(1, now - start);
  return `${Math.min(100, Math.max(0, ((Date.parse(iso) - start) / span) * 100)).toFixed(2)}%`;
}
