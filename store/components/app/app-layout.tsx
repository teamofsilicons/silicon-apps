/**
 * The app page's layout, Sidebar (Anil, 2026-11-18: picked from /playground/app-page, where the other layouts are kept
 * to experiment): the header short (the icon, the name and the tagline), then a reading column on the left (whatever
 * the page passes: screenshots, About, What's new, Compatibility, reviews) and on the right a 20rem sidebar kept in view
 * as you scroll: Open Website and the install command, the facts one under another, then the developer card. More by
 * runs across the foot. Under 1024px one column, the sidebar first, so getting the app comes right under its name.
 * After npm, GitHub, crates.io and the VS Code Marketplace. Server-rendered.
 */
import type { ReactNode } from "react";
import { AppFacts, AppGet, AppHeader } from "@/components/app/app-header";
import { DeveloperCard, MoreBy } from "@/components/app/developer";
import type { DeveloperWork } from "@/lib/developers";
import type { App } from "@/lib/types";
import styles from "./app-layout.module.css";

export function SidebarLayout({ app, work, children }: { app: App; work: DeveloperWork; children: ReactNode }) {
  return (
    <>
      <AppHeader app={app} compact />
      <div className={styles.sidebar}>
        <aside className={styles.aside} aria-label="Get it">
          <AppGet app={app} stack />
          <div className={styles.facts}><AppFacts app={app} stacked withDevelopers={false} /></div>
          <DeveloperCard app={app} work={work} />
        </aside>
        <div className={styles.main}>{children}</div>
      </div>
      {work.more.length ? <div className={styles.foot}><MoreBy app={app} work={work} /></div> : null}
    </>
  );
}
