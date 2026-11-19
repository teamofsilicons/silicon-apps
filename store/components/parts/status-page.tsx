/**
 * A page that stands in for one that isn't there or failed (app/not-found.tsx, app/error.tsx; Anil, 2026-11-19), in
 * the shell like any page: a quiet icon (or a picture across the whole page panel, the code large over it and the words on a frosted card: the
 * 404's Cosmos clouds), the page's h1, a line on what happened and what to do, the ways on (Home, All
 * Apps, Report a problem: the sidebar's report dialog), and a reference to quote where there is one. Works without
 * script (the report button is a native popover target).
 */
import type { CSSProperties, ReactNode } from "react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { REPORT_ID } from "@/components/shell/report";
import { Page } from "./page";
import styles from "./status-page.module.css";

/** `ink`: the code's colour where white would vanish into the picture (white by default). */
type Picture = { src: string; width: number; height: number; ink?: string };

export function StatusPage({ icon, picture, code, title, children, actions, reference }: { icon?: ReactNode; picture?: Picture; code: string; title: string; children: ReactNode; actions: ReactNode; reference?: string }) {
  const words = (
    <>
      <h1 id="status-title" className={styles.title}>{title}</h1>
      <p className={styles.lede}>{children}</p>
      <div className={styles.actions}>
        {actions}
        <Button type="button" variant="ghost" popoverTarget={REPORT_ID} aria-haspopup="dialog">Report a Problem</Button>
      </div>
      {reference ? <p className={styles.reference}>Reference <code>{reference}</code></p> : null}
    </>
  );
  // With a picture: it fills the whole page panel, the code large over it and the words on a frosted card.
  if (picture) {
    return (
      <section className={styles.full} aria-labelledby="status-title">
        {/* eslint-disable-next-line @next/next/no-img-element -- a static picture from public/, already sized; decorative */}
        <img className={styles.backdrop} src={picture.src} alt="" width={picture.width} height={picture.height} decoding="async" fetchPriority="high" />
        <p className={styles.bigCode} style={picture.ink ? ({ "--code-ink": picture.ink } as CSSProperties) : undefined}>{code}</p>
        <div className={styles.card} data-sq="surface">{words}</div>
      </section>
    );
  }
  return (
    <Page>
      <section className={styles.status} aria-labelledby="status-title">
        <span className={styles.icon} aria-hidden="true">{icon}</span>
        <p className={styles.code}>{code}</p>
        {words}
      </section>
    </Page>
  );
}
