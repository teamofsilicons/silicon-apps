/**
 * The home page's banner (Anil, 2026-11-14), in two styles. Several banners make the carousel (banner-carousel.tsx).
 * - "below" (home): the picture on its own across the column, then the words under it, outside the picture: a title
 *   and one line. No overlay.
 * - "overlay" (the first style, kept in /playground): the words in white on the picture's lower left, over an overlay
 *   that darkens it so they read.
 * Server-rendered; the picture is decorative (no alt text), the words carry the meaning.
 */
import styles from "./banner.module.css";

export interface BannerContent {
  id: string;
  /** The picture and where its focus is (CSS object-position). */
  picture: { src: string; width: number; height: number; focus?: string };
  title: string;
  line: string;
}

export type BannerVariant = "below" | "overlay";

function Picture({ banner, eager }: { banner: BannerContent; eager: boolean }) {
  return (
    // eslint-disable-next-line @next/next/no-img-element -- a fixed picture, served as it is.
    <img className={styles.picture} src={banner.picture.src} alt="" width={banner.picture.width} height={banner.picture.height} style={{ objectPosition: banner.picture.focus ?? "center" }} loading={eager ? "eager" : "lazy"} decoding="async" />
  );
}

export function Banner({ banner, index, count, variant = "below" }: { banner: BannerContent; index: number; count: number; variant?: BannerVariant }) {
  const label = `${index + 1} of ${count}: ${banner.title}`;
  if (variant === "overlay") {
    return (
      <div className={styles.banner} data-sq="clip" role="group" aria-roledescription="slide" aria-label={label}>
        <Picture banner={banner} eager={index === 0} />
        <span className={styles.overlay} aria-hidden="true" />
        <div className={styles.words}>
          <h2 className={styles.title}>{banner.title}</h2>
          <p className={styles.line}>{banner.line}</p>
        </div>
      </div>
    );
  }
  return (
    <div className={styles.below} role="group" aria-roledescription="slide" aria-label={label}>
      <div className={styles.frame} data-sq="clip">
        <Picture banner={banner} eager={index === 0} />
      </div>
      <div className={styles.belowWords}>
        <h2 className={styles.belowTitle}>{banner.title}</h2>
        <p className={styles.belowLine}>{banner.line}</p>
      </div>
    </div>
  );
}
