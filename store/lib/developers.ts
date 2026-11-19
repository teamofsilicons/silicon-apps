/**
 * What an app page's Developer section needs from the catalog (Anil, 2026-11-18): each author's published apps
 * (GET /v1/authors/{uuid}), so the page can say how many apps each has and show more of their work. Read-only.
 */
import { apiOrNull } from "@/lib/api";
import type { App, AuthorProfile } from "@/lib/types";

export type DeveloperWork = {
  /** How many published apps each author has, by uuid (absent where the profile didn't answer). */
  totals: Record<string, number>;
  /** The authors' other apps, every author's together, this app and repeats left out, at most `limit`. */
  more: App[];
};

export async function developerWork(app: App, limit = 6): Promise<DeveloperWork> {
  const profiles = await Promise.all(
    app.authors.map(author => apiOrNull<AuthorProfile>(`/v1/authors/${encodeURIComponent(author.uuid)}?limit=24`).catch(() => null)),
  );
  const totals: Record<string, number> = {};
  const seen = new Set([app.app_id]);
  const more: App[] = [];
  profiles.forEach((profile, index) => {
    if (!profile) return;
    totals[app.authors[index].uuid] = profile.total;
    for (const item of profile.items) {
      if (seen.has(item.app_id)) continue;
      seen.add(item.app_id);
      more.push(item);
    }
  });
  return { totals, more: more.slice(0, limit) };
}
