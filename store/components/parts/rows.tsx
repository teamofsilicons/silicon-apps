/**
 * Sections and rows, copied from the account site (silicon-accounts web, components/account/parts/rows.tsx, branch
 * compact) through Silicon Starter's copy: an h2 over one white list with a clear border, rows 8px high and 12px at the
 * sides with straight dividers, a lead (a 28px tile, or a 32px app logo), the title in body over one detail line in
 * --text-soft, and the row's buttons at its end. A LinkItem is a whole row that goes somewhere, with a chevron. From
 * Starter: a Section's `actions` beside its title, `level`, a Box (the same white box holding prose, code or a form),
 * a detail that may wrap, and a Note. The store's links are plain <a>: every page is a full server-rendered document.
 * Server components (no hooks), so they render with JavaScript off.
 */
import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import styles from "./rows.module.css";

type SectionProps = { id: string; title?: ReactNode; count?: number; label?: string; description?: ReactNode; actions?: ReactNode; tone?: "danger"; level?: 2 | 3 };

function SectionHead({ id, title, count, description, actions, level = 2 }: SectionProps) {
  const Heading = level === 3 ? "h3" : "h2";
  return (
    <div className={styles.headBlock}>
      <div className={styles.head}>
        <Heading id={id} className={styles.title}>{title}</Heading>
        {count ? <Badge size="sm" tone="neutral">{count}</Badge> : null}
        {actions ? <span className={styles.headActions}>{actions}</span> : null}
      </div>
      {description ? <p className={styles.description}>{description}</p> : null}
    </div>
  );
}

/**
 * A section: its h2 (and a count, a line under it, and actions at its right) over a boxed list. Without a title the
 * list stands alone (the page bar names it). `tone="danger"` draws the box's line in red. `listLabel` names the list.
 */
export function Section({ children, listLabel, ...props }: SectionProps & { listLabel?: string; children: ReactNode }) {
  return (
    <section className={styles.section} aria-labelledby={props.title ? props.id : undefined} aria-label={props.title ? undefined : props.label}>
      {props.title ? <SectionHead {...props} /> : null}
      <ul data-sq="surface" className={styles.rows} data-tone={props.tone} aria-label={listLabel}>{children}</ul>
    </section>
  );
}

/** The boxed list on its own, for a list that sits in a section of the page's own making (the store's results). */
export function List({ label, children }: { label?: string; children: ReactNode }) {
  return <ul data-sq="surface" className={styles.rows} aria-label={label}>{children}</ul>;
}

/** A section whose box holds prose, code or a form instead of rows. `flush` drops the box's padding (a code view). */
export function Box({ children, header, flush, sectionId, ...props }: SectionProps & { header?: ReactNode; flush?: boolean; sectionId?: string; children: ReactNode }) {
  return (
    <section id={sectionId} className={styles.section} aria-labelledby={props.title ? props.id : undefined} aria-label={props.title ? undefined : props.label}>
      {props.title ? <SectionHead {...props} /> : null}
      <div data-sq="surface" className={styles.box}>
        {header ? <div className={styles.boxHeader}>{header}</div> : null}
        <div className={styles.boxBody} data-flush={flush ? "" : undefined}>{children}</div>
      </div>
    </section>
  );
}

/** One row. `wide` lets two buttons go under the words on a phone; `wrap` lets the detail run over several lines. */
export function Row({ lead, title, detail, error, wide, wrap, children }: { lead?: ReactNode; title: ReactNode; detail?: ReactNode; error?: string | null; wide?: boolean; wrap?: boolean; children?: ReactNode }) {
  return (
    <li className={styles.row} data-wide={wide ? "" : undefined}>
      {lead}
      <span className={styles.text}>
        <span className={styles.rowTitle}>{title}</span>
        {detail ? <span className={styles.rowDetail} data-wrap={wrap ? "" : undefined}>{detail}</span> : null}
        {error ? <span className={styles.rowError} role="alert">{error}</span> : null}
      </span>
      {children ? <span className={styles.actions}>{children}</span> : null}
    </li>
  );
}

/**
 * A row inside a Section that is a link: the whole row goes to `href`, a chevron at its end; `end` sits before it.
 * `external` opens it in a new tab with an arrow instead of the chevron.
 */
/** A whole row that goes somewhere. `meta`: a third line under the detail (the store's app list: stars and systems). */
export function LinkItem({ href, lead, title, detail, meta, end, label, external, rel }: { href: string; lead?: ReactNode; title: ReactNode; detail?: ReactNode; meta?: ReactNode; end?: ReactNode; label?: string; external?: boolean; rel?: string }) {
  return (
    <li className={styles.linkItem}>
      <a href={href} className={styles.link} aria-label={label} target={external ? "_blank" : undefined} rel={rel ?? (external ? "noopener noreferrer" : undefined)}>
        {lead}
        <span className={styles.text}>
          <span className={styles.rowTitle}>{title}</span>
          {detail ? <span className={styles.rowDetail}>{detail}</span> : null}
          {meta ? <span className={styles.rowMeta}>{meta}</span> : null}
        </span>
        {end ? <span className={styles.end}>{end}</span> : null}
        <ChevronRight className={styles.chevron} size={16} strokeWidth={2} aria-hidden="true" />
      </a>
    </li>
  );
}

/** A row on its own (no section) that is a link, a chevron at its end. */
export function LinkRow({ href, lead, title, detail, end }: { href: string; lead?: ReactNode; title: ReactNode; detail?: ReactNode; end?: ReactNode }) {
  return (
    <a href={href} data-sq="surface" className={styles.linkRow}>
      {lead}
      <span className={styles.text}>
        <span className={styles.rowTitle}>{title}</span>
        {detail ? <span className={styles.rowDetail}>{detail}</span> : null}
      </span>
      {end ? <span className={styles.end}>{end}</span> : null}
      <ChevronRight className={styles.chevron} size={16} strokeWidth={2} aria-hidden="true" />
    </a>
  );
}

/** A 28px grey tile holding an icon, the lead of a row that has no mark of its own. */
export const Tile = ({ children }: { children: ReactNode }) => <span data-sq="surface" className={styles.icon} aria-hidden="true">{children}</span>;

/** The information in a detail line ("21 installs"), in the text colour. */
export const Strong = ({ children }: { children: ReactNode }) => <span className={styles.strong}>{children}</span>;

/** A section's heading on its own, for a section whose content is more than one box. */
export const Heading = SectionHead;

/** A row's message when its list is empty: one quiet line in the box. */
export const Note = ({ children }: { children: ReactNode }) => <li className={styles.note}>{children}</li>;
