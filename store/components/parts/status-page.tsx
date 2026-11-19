/**
 * A page that stands in for one that isn't there or failed (app/not-found.tsx, app/error.tsx; Anil, 2026-11-19), in
 * the shell like any page: a quiet icon (or a picture, with the code large over it: the 404's Cosmos clouds), the page's h1, a line on what happened and what to do, the ways on (Home, All
 * Apps, Report a problem: the sidebar's report dialog), and a reference to quote where there is one. Works without
 * script (the report button is a native popover target).
 */
import type { ReactNode } from "react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { REPORT_ID } from "@/components/shell/report";
import { Page } from "./page";
import styles from "./status-page.module.css";

type Picture = { src: string; width: number; height: number };

export function StatusPage({ icon, picture, code, title, children, actions, reference }: { icon?: ReactNode; picture?: Picture; code: string; title: string; children: ReactNode; actions: ReactNode; reference?: string }) {
  return (
    <Page>
      <section className={styles.status} aria-labelledby="status-title">
        {picture ? (
          <div className={styles.picture} data-sq="surface">
            {/* eslint-disable-next-line @next/next/no-img-element -- a static picture from public/, already sized; decorative */}
            <img src={picture.src} alt="" width={picture.width} height={picture.height} decoding="async" />
            <p className={styles.bigCode}>{code}</p>
          </div>
        ) : (
          <>
            <span className={styles.icon} aria-hidden="true">{icon}</span>
            <p className={styles.code}>{code}</p>
          </>
        )}
        <h1 id="status-title" className={styles.title}>{title}</h1>
        <p className={styles.lede}>{children}</p>
        <div className={styles.actions}>
          {actions}
          <Button type="button" variant="ghost" popoverTarget={REPORT_ID} aria-haspopup="dialog">Report a Problem</Button>
        </div>
        {reference ? <p className={styles.reference}>Reference <code>{reference}</code></p> : null}
      </section>
    </Page>
  );
}
