/**
 * Small server-rendered pieces every store page shares: logos and avatars, stars, the install command, tags, empty
 * states, notices, breadcrumbs, pagination and section heads. No client code: the copy buttons are wired by the one
 * script island (components/site/enhancer.tsx).
 */
import type { ReactNode } from "react";
import { headers } from "next/headers";
import { ArrowRight, ChevronLeft, ChevronRight, CircleAlert, CircleCheck, Info, Star } from "lucide-react";
import { CopyCode } from "@/components/site/code-block";
import { formatRating, initialOf, safeImage } from "@/lib/format";
import { JsonLd, type Json } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./store.module.css";

export { styles };

/** A page's own JSON-LD (the layout adds Organization and WebSite), with the request's CSP nonce. */
export async function StructuredData({ graph }: { graph: Json[] }) {
  const nonce = (await headers()).get("x-nonce") ?? undefined;
  return <JsonLd graph={graph} nonce={nonce} />;
}

export function AppLogo({ app, size = 48, decorative = true }: { app: Pick<App, "app_id" | "name" | "logo" | "logo_alt">; size?: number; decorative?: boolean }) {
  const src = safeImage(app.logo);
  if (src) {
    // Logos are the authors' own images (https or data:), shown as they are and never fetched or resized here.
    // eslint-disable-next-line @next/next/no-img-element
    return <img className={styles.logo} data-sq="clip" src={src} alt={decorative ? "" : app.logo_alt || `${app.name} logo`} width={size} height={size} style={{ width: size, height: size }} loading="lazy" decoding="async" />;
  }
  return (
    <span className={styles.monogram} data-sq="surface" style={{ width: size, height: size, fontSize: Math.round(size * 0.42) }} role={decorative ? undefined : "img"} aria-label={decorative ? undefined : `${app.name} logo`} aria-hidden={decorative ? true : undefined}>
      {initialOf(app.name)}
    </span>
  );
}

export function accountKind(id: string): "Silicon" | "Carbon" {
  return id.startsWith("si:") ? "Silicon" : "Carbon";
}

export function Avatar({ name, id, size = 36 }: { name: string; id: string; size?: number }) {
  return (
    <span className={styles.avatar} data-sq="surface" data-kind={accountKind(id)} aria-hidden="true" style={{ width: size, height: size, fontSize: Math.round(size * 0.4) }}>
      {initialOf(name.replace(/^(c|si):/, ""))}
    </span>
  );
}

export function Stars({ rating, size = 14, label }: { rating: number; size?: number; label?: string }) {
  const rounded = Math.round(rating * 2) / 2;
  return (
    <span className={styles.stars} role="img" aria-label={label ?? `${formatRating(rating)} out of 5 stars`}>
      {[1, 2, 3, 4, 5].map(n => (
        <Star key={n} size={size} strokeWidth={1.75} aria-hidden="true" className={n <= rounded ? styles.starOn : n - 0.5 === rounded ? styles.starHalf : styles.star} />
      ))}
    </span>
  );
}

/** One command on one line with a copy button (shown only with script; the text is always selectable). */
/** `wrap` lets the command break onto more lines instead of scrolling, for narrow columns like the app page's side cards. */
export function Command({ value, label, large = false, wrap = false, id }: { value: string; label?: string; large?: boolean; wrap?: boolean; id?: string }) {
  return (
    <div className={[styles.command, large ? styles.commandLarge : "", wrap ? styles.commandWrap : ""].filter(Boolean).join(" ")} data-sq="surface" id={id}>
      <code className={styles.commandText} tabIndex={0}>
        <span className={styles.prompt} aria-hidden="true">$ </span>
        {value}
      </code>
      <CopyCode label={label ?? `Copy ${value}`} value={value} />
    </div>
  );
}

export function TagList({ tags, label = "Tags", current }: { tags: string[]; label?: string; current?: string }) {
  if (!tags.length) return null;
  return (
    <ul className={styles.chips} role="list" aria-label={label}>
      {tags.map(tag => (
        <li key={tag}>
          <a className={styles.chip} data-sq="surface" href={`/search?tag=${encodeURIComponent(tag.toLowerCase())}`} aria-current={current === tag.toLowerCase() ? "page" : undefined}>
            {tag}
          </a>
        </li>
      ))}
    </ul>
  );
}

export function EmptyState({ icon, title, children, actions, headingLevel = 2 }: { icon: ReactNode; title: string; children?: ReactNode; actions?: ReactNode; headingLevel?: 1 | 2 | 3 }) {
  const Heading = headingLevel === 1 ? "h1" : headingLevel === 2 ? "h2" : "h3";
  return (
    <div className={styles.empty} data-sq="surface">
      <span className={styles.emptyIcon} data-sq="surface" aria-hidden="true">{icon}</span>
      <Heading className={styles.emptyTitle}>{title}</Heading>
      {children ? <div className={styles.emptyCopy}>{children}</div> : null}
      {actions ? <div className={styles.emptyActions}>{actions}</div> : null}
    </div>
  );
}

export function Notice({ tone = "info", title, children }: { tone?: "info" | "success" | "danger"; title?: string; children: ReactNode }) {
  const Icon = tone === "danger" ? CircleAlert : tone === "success" ? CircleCheck : Info;
  return (
    <div className={`${styles.notice} ${styles[tone]}`} data-sq="surface" role={tone === "danger" ? "alert" : "status"}>
      <Icon size={18} strokeWidth={1.75} aria-hidden="true" />
      <div className={styles.noticeBody}>
        {title ? <p className={styles.noticeTitle}>{title}</p> : null}
        {children}
      </div>
    </div>
  );
}

export function Breadcrumbs({ items }: { items: Array<{ name: string; href?: string }> }) {
  return (
    <nav aria-label="Breadcrumb" className={styles.crumbs}>
      <ol>
        {items.map((item, index) => (
          <li key={`${item.name}-${index}`}>
            {item.href && index < items.length - 1 ? <a href={item.href}>{item.name}</a> : <span aria-current="page">{item.name}</span>}
          </li>
        ))}
      </ol>
    </nav>
  );
}

export function SectionHead({ id, title, children, more, eyebrow }: { id: string; title: string; children?: ReactNode; more?: { href: string; label: string }; eyebrow?: string }) {
  return (
    <div className={styles.sectionHead}>
      <div className={styles.sectionHeadText}>
        {eyebrow ? <p className={styles.eyebrow}>{eyebrow}</p> : null}
        <h2 id={id} className={styles.sectionTitle}>{title}</h2>
        {children ? <p className={styles.sectionLede}>{children}</p> : null}
      </div>
      {more ? (
        <a className={styles.moreLink} href={more.href}>
          {more.label}
          <ArrowRight size={15} strokeWidth={1.75} aria-hidden="true" />
        </a>
      ) : null}
    </div>
  );
}

/** Previous, numbered and next links. Plain anchors: every page of results has its own URL. */
export function Pagination({ page, pages, href, label = "Pages" }: { page: number; pages: number; href: (page: number) => string; label?: string }) {
  if (pages <= 1) return null;
  const numbers = [...new Set([1, pages, page - 1, page, page + 1].filter(n => n >= 1 && n <= pages))].sort((a, b) => a - b);
  return (
    <nav className={styles.pagination} aria-label={label}>
      {page > 1 ? (
        <a className={styles.pageLink} data-sq="surface" href={href(page - 1)} rel="prev"><ChevronLeft size={16} strokeWidth={1.75} aria-hidden="true" />Previous</a>
      ) : (
        <span className={styles.pageLink} data-sq="surface" aria-disabled="true"><ChevronLeft size={16} strokeWidth={1.75} aria-hidden="true" />Previous</span>
      )}
      <ol>
        {numbers.map((n, index) => (
          <li key={n}>
            {index > 0 && n - numbers[index - 1] > 1 ? <span className={styles.pageGap} aria-hidden="true">…</span> : null}
            {n === page ? (
              <span className={styles.pageLink} data-sq="surface" aria-current="page"><span className="sr-only">Page </span>{n}</span>
            ) : (
              <a className={styles.pageLink} data-sq="surface" href={href(n)}><span className="sr-only">Page </span>{n}</a>
            )}
          </li>
        ))}
      </ol>
      {page < pages ? (
        <a className={styles.pageLink} data-sq="surface" href={href(page + 1)} rel="next">Next<ChevronRight size={16} strokeWidth={1.75} aria-hidden="true" /></a>
      ) : (
        <span className={styles.pageLink} data-sq="surface" aria-disabled="true">Next<ChevronRight size={16} strokeWidth={1.75} aria-hidden="true" /></span>
      )}
    </nav>
  );
}
