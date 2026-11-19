/**
 * /apps: All Apps (Anil, 2026-11-14), organised by category: every app the visitor can see (public apps, and private ones
 * shared with them; never drafts), in categories (lib/categories.ts), each a chapter with its title and line over
 * its first six picture cards and "See all" its count, which opens the category's own page (/categories/{id}), so
 * the page stays an overview however many apps there are; the cards (components/gallery/gallery.tsx), most installed first. Under the title, a row of links to
 * each chapter with its count. In the sidebar as "All Apps". Old /apps?… search links still go to the home page
 * (proxy.ts). Server-rendered.
 */
import type { Metadata } from "next";
import { Gallery } from "@/components/gallery/gallery";
import { Page } from "@/components/parts/page";
import { listAll } from "@/lib/catalog";
import { categorize, CHAPTER_LIMIT } from "@/lib/categories";
import { pageMetadata } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./apps.module.css";

export const metadata: Metadata = pageMetadata({
  title: "All Apps",
  description: "Every app on Silicon Apps, by what it helps you do. Each one installs with one command and keeps itself up to date.",
  path: "/apps",
});

export default async function AllAppsPage() {
  const apps = await listAll().catch((): App[] => []);
  const categories = categorize(apps);
  return (
    <Page wide>
      <div className={styles.head}>
        <div className={styles.titleRow}>
          <h1 className={styles.title}>All Apps</h1>
          <p className={styles.count}>{apps.length === 1 ? "1 app" : `${apps.length} apps`}</p>
        </div>
        <nav className={styles.jump} aria-label="Categories">
          {categories.map(category => (
            <a key={category.id} href={`#${category.id}`} className={styles.chip} data-sq="surface">
              {category.title} <span className={styles.chipCount}>{category.apps.length}</span>
            </a>
          ))}
        </nav>
      </div>
      {categories.map(category => (
        <section key={category.id} id={category.id} className={styles.chapter} aria-labelledby={`${category.id}-title`}>
          <div className={styles.chapterHead}>
            <div className={styles.chapterWords}>
              <h2 id={`${category.id}-title`} className={styles.chapterTitle}>{category.title}</h2>
              <p className={styles.chapterLine}>{category.line}</p>
            </div>
            {category.id !== "more" ? <a href={`/categories/${category.id}`} className={styles.seeAll}>See all {category.apps.length}</a> : null}
          </div>
          <Gallery apps={category.id === "more" ? category.apps : category.apps.slice(0, CHAPTER_LIMIT)} label={category.title} />
        </section>
      ))}
    </Page>
  );
}
