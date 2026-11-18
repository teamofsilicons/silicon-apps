/**
 * The apps as a grid (Anil, 2026-11-17), on All Apps and a category's grid view. Built for finding an app, not for
 * browsing pictures: columns (as many as fit at 20rem or more, three on a wide page) of open rows with no box and no cover, each the app's icon, its name, its tagline (the authors' line for cards, at most 60 characters; else the first sentence
 * of its description, whole, wrapping as it needs), between hairlines. The whole row is a link to the app.
 * Ratings are left off for now (Anil, 2026-11-17: most apps have none yet), and so are the systems' marks (Anil,
 * 2026-11-17; components/home/os-icons.tsx, still used by the list view). Server-rendered.
 */
import { AppMark } from "@/components/home/app-mark";
import { withoutName } from "@/components/home/feature";
import { withAppIcon } from "@/lib/app-icons";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./gallery.module.css";

/** The description's first sentence, whole (Anil, 2026-11-17: never cut mid-sentence), without the app's name at its
 * start where that reads well. Publishing asks for 200 to 600 characters in all; the first sentences run 37 to 191. */
export function firstSentence(app: App): string {
  const text = app.description.replace(/\s+/g, " ").trim();
  const [first] = text.split(/(?<=[.!?])\s/);
  return first ? withoutName(app.name, first) : "";
}

function Card({ app }: { app: App }) {
  return (
    <li className={styles.card}>
      <a href={appPath(app.app_id)} className={styles.link}>
        <AppMark app={withAppIcon(app)} size={44} />
        <span className={styles.words}>
          <span className={styles.name}>{app.name}</span>
          <span className={styles.line}>{app.tagline?.trim() || firstSentence(app) || "No description yet."}</span>
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
