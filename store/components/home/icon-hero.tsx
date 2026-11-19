/**
 * The icon wall banner, off the home page (kept in /playground, 2026-11-14: cluttered, and the same icons are in the
 * list below): the store itself, as a wall of
 * its apps' icons (Anil's icons where there are some, else each app's own logo, from the live catalog) floating in
 * three staggered rows on one of the account site's soft gradients (public/banner/bg-102.webp). Decorative: the icons
 * are the same apps listed below, so it is hidden from screen readers. Server-rendered, no script.
 */
import { withAppIcon } from "@/lib/app-icons";
import { safeImage } from "@/lib/format";
import type { App } from "@/lib/types";
import styles from "./icon-hero.module.css";

const ROWS = 3;
const PER_ROW = 7;

export function IconHero({ apps }: { apps: App[] }) {
  // Apps with an icon of Anil's first, then the most installed with a logo of their own.
  const marked = [...apps]
    .map(app => withAppIcon(app))
    .filter(app => safeImage(app.logo))
    .sort((a, b) => Number(b.logo.startsWith("/icons/")) - Number(a.logo.startsWith("/icons/")) || b.installs - a.installs)
    .slice(0, ROWS * PER_ROW);
  const rows = Array.from({ length: ROWS }, (_, row) => marked.filter((_, index) => index % ROWS === row));
  return (
    <div className={styles.hero} data-sq="clip" aria-hidden="true">
      <div className={styles.wall}>
        {rows.map((row, index) => (
          <div key={index} className={styles.row} data-offset={index % 2 ? "" : undefined}>
            {row.map(app => (
              // eslint-disable-next-line @next/next/no-img-element -- the apps' own icons, shown as they are.
              <img key={app.app_id} className={styles.icon} data-sq="clip" src={safeImage(app.logo)} alt="" width={64} height={64} decoding="async" />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
