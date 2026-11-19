/**
 * The bar at the top of a page, copied from the account site (components/account/parts/page-bar.tsx) as Silicon
 * Starter has it: a link back to the parent on nested pages (a page in the sidebar has none, so the bar is one line
 * shorter), the page's title on the line under it, and the page's supporting buttons on the title's right, pinned to
 * the top of the page box with a line under it. The current page is the title, so its crumb is hidden: Silicon UI's
 * Breadcrumb always ends on the current page. The title is the page's h1 role, as on every page.
 */
import type { ReactNode } from "react";
import { Breadcrumb } from "@teamofsilicons/silicon-ui/breadcrumb";
import styles from "./page-bar.module.css";

export function PageBar({ parent, title, titleId, actions }: { parent?: { label: string; href: string }; title: ReactNode; titleId?: string; actions?: ReactNode }) {
  return (
    <div className={styles.bar}>
      {parent ? <Breadcrumb items={[parent, { label: typeof title === "string" ? title : "" }]} /> : null}
      <div className={styles.row}>
        <h1 id={titleId} className={styles.title}>{title}</h1>
        {actions}
      </div>
    </div>
  );
}
