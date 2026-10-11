/**
 * Apps in lists (server-rendered), in the quiet style of Silicon Writings: the logo, the name, the app id in the pixel
 * face, a short description and the app at a glance (rating, installs, systems). The name is a card's one link; the
 * whole card is its target. Featured cards add the banner (or a paper panel with the logo) on top. AppIndex is the
 * numbered list of the home page: one line per app, with installs or the day it arrived on the right.
 */
import { Lock, Star } from "lucide-react";
import { formatCount, formatRating, operatingSystems, safeImage, truncate } from "@/lib/format";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import { AppLogo, styles } from "./parts";

export function AppMeta({ app }: { app: App }) {
  const systems = operatingSystems(app.targets);
  return (
    <ul className={styles.meta} role="list" aria-label="At a glance">
      <li>
        {app.review_count > 0 && app.rating !== null ? (
          <>
            <Star size={12} strokeWidth={1.75} aria-hidden="true" className={styles.metaStar} />
            <span>
              {formatRating(app.rating)}
              <span className="sr-only"> out of 5 from {app.review_count} {app.review_count === 1 ? "review" : "reviews"}</span>
            </span>
          </>
        ) : (
          <span>No reviews yet</span>
        )}
      </li>
      <li>
        <span>
          {formatCount(app.installs)} {app.installs === 1 ? "install" : "installs"}
        </span>
      </li>
      {systems.length ? <li>{systems.join(", ")}</li> : null}
    </ul>
  );
}

function PrivateBadge() {
  return <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface"><Lock size={11} strokeWidth={2} aria-hidden="true" />Private</span>;
}

function Title({ app, level }: { app: App; level: 2 | 3 }) {
  const Heading = level === 2 ? "h2" : "h3";
  return (
    <div className={styles.cardTitle}>
      <Heading className={styles.cardName}>
        <a href={appPath(app.app_id)}>{app.name}</a>
      </Heading>
      <p className={styles.cardId}>
        <span>{app.app_id}</span>
        {app.visibility === "private" ? <PrivateBadge /> : null}
      </p>
    </div>
  );
}

export function AppCard({ app, headingLevel = 3 }: { app: App; headingLevel?: 2 | 3 }) {
  return (
    <article className={styles.card} data-sq="surface">
      <div className={styles.cardTop}>
        <AppLogo app={app} size={48} />
        <Title app={app} level={headingLevel} />
      </div>
      <p className={styles.cardText}>{truncate(app.description || "No description yet.", 180)}</p>
      <AppMeta app={app} />
    </article>
  );
}

export function FeaturedCard({ app }: { app: App }) {
  const banner = safeImage(app.banner);
  return (
    <article className={`${styles.card} ${styles.featured}`} data-sq="surface">
      <div className={styles.featuredArt} aria-hidden="true">
        {banner ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={banner} alt="" loading="lazy" decoding="async" />
        ) : (
          <AppLogo app={app} size={64} />
        )}
      </div>
      <div className={`${styles.cardTop} ${styles.featuredTop}`}>
        {banner ? <AppLogo app={app} size={40} /> : null}
        <Title app={app} level={3} />
      </div>
      <p className={styles.cardText}>{truncate(app.description || "No description yet.", 200)}</p>
      <AppMeta app={app} />
    </article>
  );
}

export function AppGrid({ apps, headingLevel = 3, label }: { apps: App[]; headingLevel?: 2 | 3; label?: string }) {
  return (
    <ul className={styles.grid} role="list" aria-label={label}>
      {apps.map(app => (
        <li key={app.app_id}>
          <AppCard app={app} headingLevel={headingLevel} />
        </li>
      ))}
    </ul>
  );
}

const SHORT_DAY = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", timeZone: "UTC" });
const shortDay = (iso: string) => {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "" : SHORT_DAY.format(date);
};

/** One line per app: an optional number, the logo, the name and a one-line description, and installs or a date. */
export function AppIndex({ apps, numbered = false, trailing, label }: { apps: App[]; numbered?: boolean; trailing: "installs" | "date"; label?: string }) {
  return (
    <ol className={styles.index} role="list" aria-label={label}>
      {apps.map((app, index) => (
        <li key={app.app_id} className={styles.indexRow}>
          {numbered ? <span className={styles.indexNumber} aria-hidden="true">{String(index + 1).padStart(2, "0")}</span> : null}
          <AppLogo app={app} size={36} />
          <div className={styles.indexText}>
            <h3 className={styles.indexName}>
              <a href={appPath(app.app_id)}>{app.name}</a>
              {app.visibility === "private" ? <PrivateBadge /> : null}
            </h3>
            <p className={styles.indexLine}>{app.description || "No description yet."}</p>
          </div>
          {trailing === "installs" ? (
            <span className={styles.indexMeta}>{formatCount(app.installs)}<span className={styles.indexUnit}> {app.installs === 1 ? "install" : "installs"}</span></span>
          ) : (
            <time className={styles.indexMeta} dateTime={app.created_at}>{shortDay(app.created_at)}</time>
          )}
        </li>
      ))}
    </ol>
  );
}
