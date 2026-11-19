/**
 * A page that stands in for one that failed (app/error.tsx; the 404 is a gallery wall, art-wall.tsx; Anil, 2026-11-19), in
 * the shell like any page: a quiet icon, the page's h1, a line on what happened and what to do, the ways on (Home, All
 * Apps, Report a problem: the sidebar's report dialog), and a reference to quote where there is one. Works without
 * script (the report button is a native popover target).
 */
import type { ReactNode } from "react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { REPORT_ID } from "@/components/shell/report";
import { Page } from "./page";
import styles from "./status-page.module.css";

export function StatusPage({ icon, code, title, children, actions, reference }: { icon: ReactNode; code: string; title: string; children: ReactNode; actions: ReactNode; reference?: string }) {
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
