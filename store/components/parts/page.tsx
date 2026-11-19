/**
 * The page container, after the account site's layout Page (components/foundation/layout/layout.tsx): a narrow centred
 * column (--page-narrow, 896px) for most pages, or the column beside an aside, wide from the start, for content that
 * supports the page (an app's facts). Gutters and the room at the end come from tokens. Under 1024px an aside goes
 * below the column.
 */
import type { ReactNode } from "react";
import styles from "./page.module.css";

/** `wide`: the full page width (--page-max) and a page's room above, for an editorial page (the store's home). */
export function Page({ aside, asideLabel, wide = false, children }: { aside?: ReactNode; asideLabel?: string; wide?: boolean; children?: ReactNode }) {
  if (aside === undefined) return <div className={styles.page} data-layout={wide ? "wide" : undefined}>{children}</div>;
  return (
    <div className={styles.page} data-layout="aside">
      <div className={styles.columns}>
        <div className={styles.mainColumn}>{children}</div>
        <aside className={styles.sideColumn} aria-label={asideLabel}>{aside}</aside>
      </div>
    </div>
  );
}

/** A page's opening lines under its bar: a caption-role kicker, the lede in --text-soft, and links or buttons. */
export function Intro({ kicker, children, actions }: { kicker?: ReactNode; children?: ReactNode; actions?: ReactNode }) {
  return (
    <div className={styles.intro}>
      {kicker ? <p className={styles.kicker}>{kicker}</p> : null}
      {children ? <p className={styles.lede}>{children}</p> : null}
      {actions ? <div className={styles.introActions}>{actions}</div> : null}
    </div>
  );
}
