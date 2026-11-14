/**
 * The picture banner, off the home page (kept in /playground, 2026-11-14; it didn't say "apps"): pictures across the column, nothing on them, standing for Silicon
 * Apps until its own is made. Two pictures from Cosmos take turns on a timer, cross-fading, with no controls and no
 * sign of how many there are: a sky of dithered pixel clouds (public/home/hero-1.webp, uploaded by IKIGAIAIR) and a
 * pastel dithered collage of sky, dashes and a pink cloud (hero-2.webp, uploader not named). As with every Cosmos
 * picture here, their rights need checking before shipping. Pure CSS (the second fades in and out over the first), so
 * it needs no script; with reduced motion only the first shows. Decorative, so no alt text. Server-rendered.
 */
import styles from "./picture-hero.module.css";

const PICTURES = [
  { src: "/home/hero-1.webp", width: 1600, height: 905 },
  { src: "/home/hero-2.webp", width: 2000, height: 1125 },
];

export function PictureHero() {
  return (
    <div className={styles.hero} data-sq="clip" aria-hidden="true">
      {PICTURES.map((picture, index) => (
        // eslint-disable-next-line @next/next/no-img-element -- fixed pictures, served as they are.
        <img key={picture.src} className={styles.picture} src={picture.src} alt="" width={picture.width} height={picture.height} decoding="async" fetchPriority={index === 0 ? "high" : "low"} />
      ))}
    </div>
  );
}
