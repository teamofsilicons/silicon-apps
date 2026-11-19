/**
 * The 404 as a small gallery wall (Anil, 2026-11-19: redesign the 404 around art the Team likes, with credit, and a
 * cool note). In the shell like any page, on the quiet ground:
 * - the artwork hung on the wall, whole (never cropped), on a white mat with a soft shadow, as a print would be;
 * - beside it (under it on a phone) the note: "404" as a kicker, the h1, a few lines on what happened and why there
 *   is a picture here, the ways on (Go Home, Browse All Apps, Report a Problem);
 * - under the note, the wall label, as a museum's: the title, the artist, where and when, the medium, and links to the
 *   work and the artist (in a new tab).
 * The piece is lib/art.ts's first for now. Server-rendered; works without script.
 */
import type { ReactNode } from "react";
import { ArrowUpRight } from "lucide-react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { REPORT_ID } from "@/components/shell/report";
import type { Artwork } from "@/lib/art";
import styles from "./art-wall.module.css";

export function ArtWall({ art, kicker, title, children, actions }: { art: Artwork; kicker: string; title: string; children: ReactNode; actions: ReactNode }) {
  return (
    <section className={styles.wall} aria-labelledby="status-title">
      <figure className={styles.piece}>
        <div className={styles.mat}>
          {/* eslint-disable-next-line @next/next/no-img-element -- the artist's picture as given, never resized or recompressed */}
          <img src={art.picture.src} alt={art.picture.alt} width={art.picture.width} height={art.picture.height} decoding="async" fetchPriority="high" />
        </div>
        <figcaption className="sr-only">{art.title} by {art.artist}, {art.made}.</figcaption>
      </figure>
      <div className={styles.side}>
        <div className={styles.note}>
          <p className={styles.kicker}>{kicker}</p>
          <h1 id="status-title" className={styles.title}>{title}</h1>
          <div className={styles.lede}>{children}</div>
          <div className={styles.actions}>
            {actions}
            <Button type="button" variant="ghost" popoverTarget={REPORT_ID} aria-haspopup="dialog">Report a Problem</Button>
          </div>
        </div>
        <aside className={styles.label} aria-label="About the artwork">
          <p className={styles.labelKicker}>On the wall</p>
          <p className={styles.artist}>{art.artist}</p>
          <p className={styles.work}><cite>{art.title}</cite>, {art.made}</p>
          {art.medium ? <p className={styles.medium}>{art.medium}</p> : null}
          <p className={styles.links}>
            <a href={art.href} target="_blank" rel="noopener noreferrer">See the print<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /></a>
            <a href={art.artistHref} target="_blank" rel="noopener noreferrer">More by {art.artist.split(" ")[0]}<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /></a>
          </p>
        </aside>
      </div>
    </section>
  );
}
