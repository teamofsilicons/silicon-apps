/**
 * The 404 as a gallery wall (Anil, 2026-11-19, then cut to the least): a column on white, read like an article's
 * opening: the h1 ("404" muted over the words) on the left with Go Home at its foot on the right, then a piece the
 * Team likes (lib/art.ts) whole across the column, rounded with a faint border, its credit under it on the left (a
 * link to the work), as a caption. On a phone Go Home goes under the h1.
 * Server-rendered.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import type { Artwork } from "@/lib/art";
import styles from "./art-wall.module.css";

export function ArtWall({ art, title }: { art: Artwork; title: string }) {
  return (
    <section className={styles.wall} aria-labelledby="status-title">
      <header className={styles.note}>
        <h1 id="status-title" className={styles.title}><span className={styles.code}>404</span> {title}</h1>
        <ButtonLink href="/" variant="secondary">Go Home</ButtonLink>
      </header>
      <figure className={styles.piece}>
        {/* eslint-disable-next-line @next/next/no-img-element -- the artist's picture as given, never resized or recompressed */}
        <img src={art.picture.src} alt={art.picture.alt} width={art.picture.width} height={art.picture.height} decoding="async" fetchPriority="high" />
        <figcaption className={styles.credit}>
          <a href={art.href} target="_blank" rel="noopener noreferrer"><cite>{art.title}</cite>, {art.artist}</a>
        </figcaption>
      </figure>
    </section>
  );
}
