/**
 * The All Apps gallery (Anil, 2026-11-14): every app the visitor can see as a picture card, for browsing rather than
 * scanning a list. A card is a cover (the app's own picture where it has one, else its banner or first picture, else
 * its icon large on one of the account site's soft pictures, chosen by its id so it stays the same), the app's icon
 * rising into the cover's lower-left corner, its name, its first sentence, and its rating and systems. The whole card
 * is a link to the app. Server-rendered.
 */
import { AppMark } from "@/components/home/app-mark";
import { limitWords, withoutName } from "@/components/home/feature";
import { OsIcons } from "@/components/home/os-icons";
import { withAppIcon } from "@/lib/app-icons";
import { APP_PICTURES } from "@/lib/app-pictures";
import { formatRating, safeImage } from "@/lib/format";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./gallery.module.css";

const GROUNDS = ["/banner/bg-110.webp", "/banner/bg-102.webp", "/banner/bg-127.webp", "/banner/bg-139.webp"];

/** The same ground for an app every time: its id, hashed. */
function groundOf(appId: string): string {
  let hash = 0;
  for (const char of appId) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return GROUNDS[hash % GROUNDS.length];
}

/** The app's own picture, else its banner, else its first picture; none when it has nothing of its own. */
function pictureOf(app: App): { src: string; focus?: string } | null {
  const own = APP_PICTURES[app.app_id];
  if (own) return own;
  const banner = safeImage(app.banner);
  if (banner) return { src: banner };
  const first = (app.carousel ?? []).find(item => item.kind !== "video" && safeImage(item.url));
  return first ? { src: safeImage(first.url)! } : null;
}

function Card({ app }: { app: App }) {
  const marked = withAppIcon(app);
  const picture = pictureOf(app);
  return (
    <li className={styles.card}>
      <a href={appPath(app.app_id)} className={styles.link}>
        <span className={styles.cover} data-sq="clip" aria-hidden="true">
          {picture ? (
            // eslint-disable-next-line @next/next/no-img-element -- the app's own picture, shown as it is.
            <img src={picture.src} alt="" style={{ objectPosition: picture.focus ?? "center" }} loading="lazy" decoding="async" />
          ) : (
            <span className={styles.ground} style={{ backgroundImage: `url("${groundOf(app.app_id)}")` }}>
              <span className={styles.bigIcon}><AppMark app={marked} size={64} /></span>
            </span>
          )}
        </span>
        <span className={styles.icon} aria-hidden="true"><AppMark app={marked} size={40} /></span>
        <span className={styles.words}>
          <span className={styles.name}>{app.name}</span>
          <span className={styles.line}>{app.description ? limitWords(withoutName(app.name, app.description), 12) : "No description yet."}</span>
          <span className={styles.meta}>
            {app.rating !== null ? <span aria-label={`${formatRating(app.rating)} out of 5 stars`}>{formatRating(app.rating)} <span className={styles.star} aria-hidden="true">★</span></span> : <span>No ratings yet</span>}
            <OsIcons targets={app.targets} />
          </span>
        </span>
      </a>
    </li>
  );
}

export function Gallery({ apps, label }: { apps: App[]; label: string }) {
  return (
    <ul className={styles.gallery} aria-label={label}>
      {apps.map(app => <Card key={app.app_id} app={app} />)}
    </ul>
  );
}
