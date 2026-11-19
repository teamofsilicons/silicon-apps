/**
 * An app page's What's new, Store (Anil, 2026-11-18: picked from the lab at /playground/whats-new; after the App
 * Store's), after About:
 * - its title (h2), then "Version 5.3.1" and how long ago on one line (the date on hover);
 * - the notes cut to three lines with "more" under them; "more" opens the full notes (a details, so no script);
 * - folds, each opened by its title with Lucide's triangle before it (pointing right, down when open): Version history
 *   (every release, newest first: version, date, notes), Withdrawn releases (where there are any: struck through, the
 *   date and the developer's reason) and Other ways to install (the development channel, newer than production
 *   where it is; this exact version).
 * The history is GET /v1/apps/{id}/releases (lib/releases.ts); without it, the current release stands alone. Who signed
 * it and the systems are in the header. No section until there is a release. Server-rendered.
 */
import type { ReactNode } from "react";
import { Triangle } from "lucide-react";
import { Command } from "@/components/app/command";
import { formatDate } from "@/lib/format";
import { ago } from "@/lib/releases";
import type { App, Release } from "@/lib/types";
import styles from "./whats-new.module.css";

const channelOf = (channel: Release["channel"]) => (channel === "production" ? "Production" : "Development");

/** Whether version `a` is newer than `b`, part by part as numbers (1.0.10 after 1.0.9); a pre-release suffix is ignored. */
function newer(a: string, b: string): boolean {
  const parts = (version: string) => version.split(/[-+]/)[0].split(".").map(part => Number.parseInt(part, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let index = 0; index < Math.max(x.length, y.length); index += 1) {
    if ((x[index] ?? 0) !== (y[index] ?? 0)) return (x[index] ?? 0) > (y[index] ?? 0);
  }
  return false;
}

function Notes({ text, small = false }: { text: string; small?: boolean }) {
  const paragraphs = text.split(/\n{2,}/).map(paragraph => paragraph.trim()).filter(Boolean);
  if (!paragraphs.length) return <p className={styles.quiet}>No notes for this release.</p>;
  return <div className={styles.notes} data-small={small ? "" : undefined}>{paragraphs.map((paragraph, index) => <p key={index}>{paragraph}</p>)}</div>;
}

/** A fold: its title after Lucide's triangle (right, down when open), an optional count, then what it holds. */
function Fold({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <details className={styles.fold}>
      <summary>
        <Triangle className={styles.triangle} size={10} strokeWidth={2} fill="currentColor" aria-hidden="true" />
        {title}
        {count !== undefined ? <span className={styles.count}>{count}</span> : null}
      </summary>
      <div className={styles.foldBody}>{children}</div>
    </details>
  );
}

export function WhatsNew({ app, history = [] }: { app: App; history?: Release[] }) {
  const current = app.latest_production ?? app.latest_development;
  if (!current) return null;
  const releases = history.length ? history : [current];
  const dev = app.latest_development;
  const newerDev = app.latest_production && dev && newer(dev.version, app.latest_production.version) ? dev : null;
  const withdrawn = app.withdrawn_releases ?? [];

  return (
    <section className={styles.section} aria-labelledby="whats-new-title">
      <h2 id="whats-new-title" className={styles.title}>What’s new</h2>
      <p className={styles.meta}>
        <span>Version {current.version}</span>
        <time dateTime={current.created_at} title={formatDate(current.created_at)}>{ago(current.created_at)}</time>
      </p>
      <details className={styles.clamp}>
        <summary>
          <span className={styles.clampText}>{current.notes.trim() || "No notes for this release."}</span>
          <span className={styles.clampMore}>more</span>
        </summary>
        <Notes text={current.notes} />
      </details>

      <div className={styles.folds}>
        {releases.length > 1 ? (
          <Fold title="Version history" count={releases.length}>
            <ol className={styles.history}>
              {releases.map(release => (
                <li key={release.id}>
                  <p className={styles.historyHead}><span className={styles.version}>{release.version}</span><time dateTime={release.created_at}>{formatDate(release.created_at)}</time></p>
                  <Notes text={release.notes} small />
                </li>
              ))}
            </ol>
          </Fold>
        ) : null}
        {withdrawn.length ? (
          <Fold title="Withdrawn releases" count={withdrawn.length}>
            <ul className={styles.history}>
              {withdrawn.map(item => (
                <li key={item.release_id}>
                  <p className={styles.historyHead}><span className={`${styles.version} ${styles.struck}`}>{item.version} · {channelOf(item.channel)}</span><time dateTime={item.withdrawn_at}>Withdrawn {formatDate(item.withdrawn_at)}</time></p>
                  {item.reason ? <p className={styles.reason}>{item.reason}</p> : null}
                </li>
              ))}
            </ul>
            <p className={styles.quiet}>A withdrawn release is never installed again, and updates move every copy of it to the latest good release.</p>
          </Fold>
        ) : null}
        <Fold title="Other ways to install">
          <div className={styles.ways}>
            {dev ? (
              <div className={styles.way}>
                <p>{newerDev ? `The development release, ${newerDev.version}, newer than production` : "The latest development release"}</p>
                <Command value={`silicon-apps install '${app.app_id}>dev'`} label="Copy the development install command" />
              </div>
            ) : null}
            <div className={styles.way}>
              <p>This exact version, {current.version}</p>
              <Command value={`silicon-apps install '${app.app_id}@${current.version}'`} label="Copy the exact version install command" />
            </div>
            <p className={styles.quiet}>The CLI asks before switching between production and development. An exact version picks what you install first; updates still follow its channel.</p>
          </div>
        </Fold>
      </div>
    </section>
  );
}
