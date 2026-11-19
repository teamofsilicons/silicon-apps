/**
 * The 404 as a gallery wall (Anil, 2026-11-19, then cut to the least): one centred column on white, a piece the Team
 * likes (lib/art.ts) whole and flat, its credit under it (a link to the work), then "404", the h1 and Go Home.
 * Server-rendered.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import type { Artwork } from "@/lib/art";
import styles from "./art-wall.module.css";

export function ArtWall({ art, title }: { art: Artwork; title: string }) {
  return (
    <section className={styles.wall} aria-labelledby="status-title">
      <figure className={styles.piece}>
        {/* eslint-disable-next-line @next/next/no-img-element -- the artist's picture as given, never resized or recompressed */}
        <img src={art.picture.src} alt={art.picture.alt} width={art.picture.width} height={art.picture.height} decoding="async" fetchPriority="high" />
        <figcaption className={styles.credit}>
          <a href={art.href} target="_blank" rel="noopener noreferrer"><cite>{art.title}</cite>, {art.artist}</a>
        </figcaption>
      </figure>
      <div className={styles.note}>
        <p className={styles.code}>404</p>
        <h1 id="status-title" className={styles.title}>{title}</h1>
        <ButtonLink href="/">Go Home</ButtonLink>
      </div>
    </section>
  );
}
