/**
 * An app page's About (Anil, 2026-11-18), the section after the header: its title (h2), the description as the
 * authors wrote it (a paragraph for each blank-line break) at a reading width, then the tags as pills, each a search
 * for that tag. A quiet line where there is no description yet. Server-rendered.
 */
import type { App } from "@/lib/types";
import styles from "./about.module.css";

export function About({ app }: { app: App }) {
  const paragraphs = app.description.split(/\n{2,}/).map(paragraph => paragraph.trim()).filter(Boolean);
  return (
    <section className={styles.about} aria-labelledby="about-title">
      <h2 id="about-title" className={styles.title}>About</h2>
      <div className={styles.prose}>
        {paragraphs.length ? paragraphs.map((paragraph, index) => <p key={index}>{paragraph}</p>) : <p className={styles.quiet}>The developer hasn’t written a description yet.</p>}
      </div>
      {app.tags.length ? (
        <ul className={styles.tags} aria-label="Tags">
          {app.tags.map(tag => <li key={tag}><a href={`/?${new URLSearchParams({ q: tag })}`} className={styles.tag}>{tag}</a></li>)}
        </ul>
      ) : null}
    </section>
  );
}
