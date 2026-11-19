/**
 * The other designs for an app page's What's new (Anil, 2026-11-18: "go through similar works, and come up with 4 very
 * different designs"), beside the page's own (components/app/whats-new.tsx, which is the fourth, A · Store, picked 2026-11-18), compared at /playground/whats-new. Each
 * reads the app's release history (lib/releases.ts: 3 to 8 releases in the catalog, notes of 0 to 1,230 characters).
 *
 *   B · Timeline       after GitHub Releases and Linear's or Raycast's changelogs: a dated rail, newest first, each
 *                      release's version and notes; the three newest shown, the rest behind "Show all"
 *   C · Notes window   after Sparkle's update window on the Mac: a framed panel, the versions in a list on the left,
 *                      the chosen one's notes on the right (radio buttons, so it works without script)
 *   D · Pulse          after the Chrome Web Store and npm, which lead with activity: "Updated 3 weeks ago" and how
 *                      many releases since when, a strip with a dot per release in time, then the latest notes
 * Server-rendered; nothing needs script.
 */
import type { App, Release } from "@/lib/types";
import { formatDate } from "@/lib/format";
import { ago, placeInTime } from "@/lib/releases";
import styles from "./designs.module.css";

function Notes({ text, className }: { text: string; className?: string }) {
  const paragraphs = text.split(/\n{2,}/).map(paragraph => paragraph.trim()).filter(Boolean);
  if (!paragraphs.length) return <p className={styles.quiet}>No notes for this release.</p>;
  return <div className={`${styles.notes} ${className ?? ""}`}>{paragraphs.map((paragraph, index) => <p key={index}>{paragraph}</p>)}</div>;
}

function Title() {
  return <h2 id="whats-new-title" className={styles.title}>What’s new</h2>;
}

function TimelineItem({ release, latest }: { release: Release; latest: boolean }) {
  return (
    <li className={styles.tlItem}>
      <time className={styles.tlDate} dateTime={release.created_at}>{formatDate(release.created_at)}</time>
      <span className={styles.tlDot} data-latest={latest ? "" : undefined} aria-hidden="true" />
      <div className={styles.tlBody}>
        <p className={styles.tlHead}><span className={styles.tlVersion}>{release.version}</span>{latest ? <span className={styles.pill}>Latest</span> : null}</p>
        <Notes text={release.notes} className={latest ? undefined : styles.small} />
      </div>
    </li>
  );
}

export function TimelineDesign({ history }: { app: App; history: Release[] }) {
  if (!history.length) return null;
  const shown = history.slice(0, 3);
  const rest = history.slice(3);
  return (
    <section className={styles.timeline} aria-labelledby="whats-new-title">
      <Title />
      <ol className={styles.tl}>
        {shown.map((release, index) => <TimelineItem key={release.id} release={release} latest={index === 0} />)}
      </ol>
      {rest.length ? (
        <details className={styles.tlMore}>
          <summary>Show all {history.length} releases</summary>
          <ol className={styles.tl}>{rest.map(release => <TimelineItem key={release.id} release={release} latest={false} />)}</ol>
        </details>
      ) : null}
    </section>
  );
}

export function WindowDesign({ app, history }: { app: App; history: Release[] }) {
  if (!history.length) return null;
  const name = `release-${app.app_id}`;
  return (
    <section className={styles.windowSection} aria-labelledby="whats-new-title">
      <Title />
      <div className={styles.window} data-sq="surface" role="group" aria-label="Releases">
        {history.map((release, index) => (
          <div key={release.id} className={styles.winItem}>
            {/* eslint-disable-next-line no-restricted-syntax -- a bare radio drives the panels with CSS alone, so the design works without script; Silicon UI's RadioGroup needs script. */}
            <input type="radio" name={name} id={`${name}-${index}`} className={styles.winRadio} defaultChecked={index === 0} />
            <label htmlFor={`${name}-${index}`} className={styles.winTab}>
              <span className={styles.mono}>{release.version}</span>
              <span className={styles.winDate}>{index === 0 ? "Latest" : formatDate(release.created_at)}</span>
            </label>
            <div className={styles.winPanel}>
              <p className={styles.winHead}><span className={styles.winVersion}>Version {release.version}</span><time dateTime={release.created_at}>{formatDate(release.created_at)} · {ago(release.created_at)}</time></p>
              <Notes text={release.notes} />
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

export function PulseDesign({ history }: { app: App; history: Release[] }) {
  const [latest] = history;
  if (!latest) return null;
  const first = history[history.length - 1];
  const at = (iso: string) => placeInTime(iso, first.created_at);
  return (
    <section className={styles.pulse} aria-labelledby="whats-new-title">
      <Title />
      <div className={styles.pulseStats}>
        <p className={styles.pulseBig}>Updated {ago(latest.created_at)}</p>
        <p className={styles.quiet}>{history.length === 1 ? "1 release" : `${history.length} releases`} since {formatDate(first.created_at)}</p>
      </div>
      <div className={styles.strip} role="img" aria-label={`Releases over time: ${history.map(release => `${release.version} on ${formatDate(release.created_at)}`).join(", ")}`}>
        <span className={styles.track} aria-hidden="true" />
        {[...history].reverse().map((release, index, all) => (
          <span key={release.id} className={styles.tick} data-latest={index === all.length - 1 ? "" : undefined} style={{ left: at(release.created_at) }} title={`${release.version}, ${formatDate(release.created_at)}`} aria-hidden="true" />
        ))}
        <span className={styles.stripStart} aria-hidden="true">{formatDate(first.created_at)}</span>
        <span className={styles.stripEnd} aria-hidden="true">Today</span>
      </div>
      <div className={styles.pulseLatest} data-sq="surface">
        <p className={styles.historyHead}><span className={styles.tlVersion}>{latest.version}</span><time dateTime={latest.created_at}>{formatDate(latest.created_at)}</time></p>
        <Notes text={latest.notes} />
      </div>
      {history.length > 1 ? (
        <details className={styles.history}>
          <summary>Earlier releases <span className={styles.count}>{history.length - 1}</span></summary>
          <ol className={styles.compact}>
            {history.slice(1).map(release => (
              <li key={release.id}><span className={styles.mono}>{release.version}</span><span className={styles.compactNote}>{release.notes || "No notes"}</span><time dateTime={release.created_at}>{formatDate(release.created_at)}</time></li>
            ))}
          </ol>
        </details>
      ) : null}
    </section>
  );
}
