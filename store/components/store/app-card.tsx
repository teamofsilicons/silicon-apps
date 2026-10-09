/**
 * App cards (server-rendered): the logo, name, app id, a short description and the app at a glance (rating, installs,
 * systems). The name is the card's one link; the whole card is its target. Featured cards add the banner on top.
 */
import { Download, Lock, Star } from "lucide-react";
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
            <Star size={14} strokeWidth={1.75} aria-hidden="true" className={styles.metaStar} />
            <span>
              {formatRating(app.rating)}
              <span className="sr-only"> out of 5 from {app.review_count} {app.review_count === 1 ? "review" : "reviews"}</span>
            </span>
          </>
        ) : (
          <span className={styles.muted}>No reviews yet</span>
        )}
      </li>
      <li>
        <Download size={14} strokeWidth={1.75} aria-hidden="true" />
        <span>
          {formatCount(app.installs)}
          <span className="sr-only"> {app.installs === 1 ? "install" : "installs"}</span>
        </span>
      </li>
      {systems.length ? <li className={styles.metaOs}>{systems.join(", ")}</li> : null}
    </ul>
  );
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
        {app.visibility === "private" ? (
          <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface"><Lock size={11} strokeWidth={2} aria-hidden="true" />Private</span>
        ) : null}
      </p>
    </div>
  );
}

export function AppCard({ app, headingLevel = 3 }: { app: App; headingLevel?: 2 | 3 }) {
  return (
    <article className={styles.card} data-sq="surface">
      <div className={styles.cardTop}>
        <AppLogo app={app} size={52} />
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
          <>
            <span className={styles.featuredWash} />
            <AppLogo app={app} size={72} />
          </>
        )}
      </div>
      <div className={`${styles.cardTop} ${styles.featuredTop}`}>
        {banner ? <AppLogo app={app} size={44} /> : null}
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
