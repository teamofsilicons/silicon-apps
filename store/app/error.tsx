"use client";
/** The last resort when a page fails to render. Next logs the server error; nothing private is shown here. */
import buttonStyles from "@/components/arc/button/button.module.css";
import linkStyles from "@/components/foundation/button-link.module.css";
import styles from "@/components/store/store.module.css";

export default function PageError({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return (
    <div className={`${styles.page} ${styles.narrow}`}>
      <div className={styles.empty} data-sq="surface" role="alert">
        <h1 className={styles.emptyTitle}>Something went wrong on our side</h1>
        <div className={styles.emptyCopy}>
          <p>This page could not load. Try again, and if it keeps happening run <code>silicon-apps report &quot;what happened&quot;</code> so the Team hears about it.</p>
        </div>
        <div className={styles.emptyActions}>
          <button type="button" className={`${buttonStyles.button} ${buttonStyles.primary} ${buttonStyles.md}`} data-sq="surface" onClick={() => reset()}>Try again</button>
          <a className={`${buttonStyles.button} ${buttonStyles.secondary} ${buttonStyles.md} ${linkStyles.link}`} data-sq="surface" data-variant="secondary" href="/">Back to the store</a>
        </div>
      </div>
    </div>
  );
}
