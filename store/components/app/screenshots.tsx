/**
 * An app page's screenshots (Anil, 2026-11-18), between the header and About (see it before reading about it): its title (h2) and the app's carousel as one strip that
 * scrolls sideways, each picture or video in a frame of a fixed ratio by the screen it shows:
 *   desktop  16:10, a laptop's window
 *   mobile   9:19.5, a phone's screen
 * Every frame is the same height (320px; 210 on a phone), so a strip that mixes the two lines up, and a picture of
 * another ratio is cropped to its frame (object-fit: cover). Which frame comes from the item's `form` (proposed for the
 * API; absent means desktop). Videos play with their controls and load nothing until asked. Only http and https addresses
 * (and the store's own, for the lab's samples) are shown; with none, there's no section. Works without script (the strip scrolls natively, snapping to
 * each frame). Server-rendered.
 */
import { safeImage } from "@/lib/format";
import type { App, Media } from "@/lib/types";
import styles from "./screenshots.module.css";

export function Screenshots({ app, items }: { app: App; items?: Media[] }) {
  const shots = (items ?? app.carousel ?? []).flatMap(item => {
    const url = safeImage(item.url);
    return url && /^https?:|^\//.test(url) ? [{ ...item, url }] : [];
  });
  if (!shots.length) return null;
  return (
    <section className={styles.section} aria-labelledby="screenshots-title">
      <h2 id="screenshots-title" className={styles.title}>Screenshots</h2>
      <ul className={styles.strip} aria-label={`Screenshots of ${app.name}`}>
        {shots.map((shot, index) => (
          <li key={`${shot.url}-${index}`} className={styles.frame} data-form={shot.form === "mobile" ? "mobile" : "desktop"} data-sq="clip">
            {shot.kind === "video" ? (
              <video className={styles.media} src={shot.url} controls preload="none" playsInline aria-label={shot.alt || `${app.name}, video ${index + 1}`} />
            ) : (
              // eslint-disable-next-line @next/next/no-img-element -- the app's own screenshot, shown as it is.
              <img className={styles.media} src={shot.url} alt={shot.alt || `${app.name}, screenshot ${index + 1}`} loading="lazy" decoding="async" />
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
