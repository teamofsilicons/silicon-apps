/**
 * The 404 as a gallery wall (Anil, 2026-11-19, then cut to the least): one centred column on white: the h1 ("404"
 * in its own colour, then the line) and Go Home, then a piece the Team likes (lib/art.ts) whole, with rounded corners
 * and a faint border, its credit under it (a link to the work).
 * Server-rendered.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import type { Artwork } from "@/lib/art";
import styles from "./art-wall.module.css";

export function ArtWall({ art, title }: { art: Artwork; title: string }) {
  return (
    <section className={styles.wall} aria-labelledby="status-title">
      <div className={styles.note}>
        <h1 id="status-title" className={styles.title}><span className={styles.code}>404</span> {title}</h1>
        <ButtonLink href="/">Go Home</ButtonLink>
      </div>
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
