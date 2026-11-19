/**
 * The home page's feature (Anil, 2026-11-14): one element in two parts side by side, a 16:9 picture on the left (60%)
 * and the words on the right (40%), centred on the picture's height. Under 768px the words go below the picture.
 *
 * The words, in their order of importance (the hierarchy pass, 2026-11-14; the icon and "Featured" label it tried were
 * too much, Anil): the app's name at 36px; the description close under the name (at most 15
 * words at the base size, which fits three lines, without the app's name repeated at its start); then, after more
 * room, View App as a secondary button, so it no longer outweighs the name. The picture is the store's own
 * (public/home/ascii-clouds.jpg), not the app's. Server-rendered.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./feature.module.css";

/** The feature's picture (Anil, 2026-11-14): glowing ASCII clouds in pastel skies, the same for every app for now.
 * Decorative, so it has no alt text. 4:3 at 1448 × 1086, cropped to 16:9 from its middle. */
const PICTURE = "/home/ascii-clouds.jpg";

function Picture() {
  // eslint-disable-next-line @next/next/no-img-element -- a fixed picture, served as it is.
  return <img className={styles.picture} data-sq="clip" src={PICTURE} alt="" width={1448} height={1086} decoding="async" />;
}

/** The body's word limit (Anil, 2026-11-14): 15 words fit three lines of 16px text (the base size) at the narrowest the
 * words column gets beside the picture (about 285px), so the three-line clamp in the CSS is only a backstop. */
const BODY_WORDS = 15;

/**
 * The description within BODY_WORDS: whole sentences while they fit, else the first BODY_WORDS words with an ellipsis.
 */
export function limitWords(text: string, limit = BODY_WORDS): string {
  const words = text.replace(/\s+/g, " ").trim().split(" ").filter(Boolean);
  if (words.length <= limit) return words.join(" ");
  const kept = words.slice(0, limit);
  // End on the last full sentence inside the limit, when there is one.
  const sentenceEnd = kept.findLastIndex(word => /[.!?]$/.test(word));
  if (sentenceEnd >= 0) return kept.slice(0, sentenceEnd + 1).join(" ");
  return `${kept.join(" ").replace(/[,;:]$/, "")}…`;
}

/** The description without the app's name at its start, which the heading already says: "Briefcase keeps your files…"
 * becomes "Keeps your files…". Left as written when the name is the sentence's subject of "is", "are", "was" or "has"
 * ("DM is direct messaging…" would read "Is direct messaging…"), or doesn't start the description. */
export function withoutName(name: string, text: string): string {
  const trimmed = text.trim();
  if (!trimmed.toLowerCase().startsWith(`${name.toLowerCase()} `)) return trimmed;
  const rest = trimmed.slice(name.length + 1);
  if (/^(is|are|was|were|has|have)\b/i.test(rest)) return trimmed;
  return rest.charAt(0).toUpperCase() + rest.slice(1);
}

export function Feature({ app }: { app: App }) {
  return (
    <section className={styles.feature} aria-labelledby="feature-title">
      <Picture />
      <div className={styles.text}>
        <div className={styles.title}>
          <h2 id="feature-title" className={styles.heading}>{app.name}</h2>
          <p className={styles.body}>{app.description ? limitWords(withoutName(app.name, app.description)) : "The authors have not written a description yet."}</p>
        </div>
        <ButtonLink href={appPath(app.app_id)} variant="secondary">View App</ButtonLink>
      </div>
    </section>
  );
}
