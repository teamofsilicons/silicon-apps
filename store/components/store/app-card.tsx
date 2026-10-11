/**
 * The one way the store lists apps (server-rendered), as Silicon Writings lists its posts: a quiet row per app with
 * its logo, its name and id, the first sentence of what it does, and one plain line of facts in the pixel face. The name
 * is the row's one link and the whole row is its target. Home, search and author pages all use it.
 */
import { Lock } from "lucide-react";
import { formatCount, formatRating, operatingSystems, summaryOf } from "@/lib/format";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import { AppLogo, styles } from "./parts";

/** "4.5 ★ · 21 installs · macOS, Linux, Windows" as one run of text, so it wraps like a sentence. */
function facts(app: App): string {
  const parts = [
    app.review_count > 0 && app.rating !== null ? `${formatRating(app.rating)} ★` : null,
    `${formatCount(app.installs)} ${app.installs === 1 ? "install" : "installs"}`,
    operatingSystems(app.targets).join(", ") || null,
  ];
  return parts.filter(Boolean).join("  ·  ");
}

export function AppRow({ app, headingLevel = 3 }: { app: App; headingLevel?: 2 | 3 }) {
  const Heading = headingLevel === 2 ? "h2" : "h3";
  return (
    <article className={styles.row}>
      <AppLogo app={app} size={44} />
      <div className={styles.rowBody}>
        <Heading className={styles.rowName}>
          <a href={appPath(app.app_id)}>{app.name}</a>
          <span className={styles.rowId}>{app.app_id}</span>
          {app.visibility === "private" ? (
            <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface"><Lock size={10} strokeWidth={2} aria-hidden="true" />Private</span>
          ) : null}
        </Heading>
        <p className={styles.rowText}>{app.description ? summaryOf(app.description) : "No description yet."}</p>
        <p className={styles.rowFacts}>{facts(app)}</p>
      </div>
    </article>
  );
}

export function AppList({ apps, headingLevel = 3, label }: { apps: App[]; headingLevel?: 2 | 3; label?: string }) {
  return (
    <ul className={styles.list} role="list" aria-label={label}>
      {apps.map(app => (
        <li key={app.app_id}>
          <AppRow app={app} headingLevel={headingLevel} />
        </li>
      ))}
    </ul>
  );
}
