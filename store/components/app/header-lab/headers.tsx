/**
 * The other design kept for an app page's header (Anil, 2026-11-18: of the four tried, Cover and Editorial were kept;
 * Editorial became the page's own, components/app/app-header.tsx), compared with it at /playground/app-header. It says
 * the same things (parts.tsx); the layout and the ground differ. Spotlight (a centred gradient panel) and Install card
 * (a buy box on the right) were dropped and are in git.
 *
 *   B · Cover         the app's picture across the top (else a soft gradient), the icon rising out of its corner, the
 *                     words beside it and the actions on the right of the same row
 * Server-rendered.
 */
import type { CSSProperties } from "react";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { AppMark } from "@/components/home/app-mark";
import { withAppIcon } from "@/lib/app-icons";
import type { App } from "@/lib/types";
import { Actions, Authors, CategoryChip, headerFacts, Note } from "./parts";
import styles from "./headers.module.css";

function Name({ app, className }: { app: App; className: string }) {
  return (
    <h1 id="app-name" className={className}>
      {app.name}
      {app.visibility === "private" ? <Badge size="sm" tone="neutral">Private</Badge> : null}
    </h1>
  );
}

export function CoverHeader({ app }: { app: App }) {
  const facts = headerFacts(app);
  const art = facts.cover ? ({ backgroundImage: `url("${facts.cover.src}")`, backgroundPosition: facts.cover.focus ?? "center" } as CSSProperties) : undefined;
  return (
    <header className={styles.cover}>
      <div className={styles.coverArt} data-sq="clip" data-fallback={facts.cover ? undefined : ""} style={art} aria-hidden="true" />
      <div className={styles.coverBody}>
        <span className={styles.coverIcon}><AppMark app={withAppIcon(app)} size={96} /></span>
        <div className={styles.coverRow}>
          <div className={styles.coverWords}>
            <Name app={app} className={styles.coverName} />
            {facts.line ? <p className={styles.coverLine}>{facts.line}</p> : null}
            <p className={styles.meta}><Authors app={app} /><CategoryChip facts={facts} /></p>
          </div>
          <div className={styles.coverGet}>
            <Actions app={app} facts={facts} />
            <Note facts={facts} />
          </div>
        </div>
      </div>
    </header>
  );
}
