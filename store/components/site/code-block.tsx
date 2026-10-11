/**
 * A code block on the store's pages, as on the developer site (developer/components/docs/code-block.tsx): Arc's
 * code-block anatomy (a squircle panel, a header with its title and a copy button, the source in the monospace face),
 * highlighted on the server (lib/docs/highlight.ts, the developer site's highlighter) so the page ships plain spans.
 * The copy button is wired by the store's one script island (components/site/enhancer.tsx) through its data
 * attributes; without script it stays hidden ([data-js-only]) and the command is plain, selectable text.
 */
import { highlight, languageLabel } from "@/lib/docs/highlight";
import styles from "./code-block.module.css";
import { CopyCode } from "./copy-code";

export { CopyCode };

/** `wrap` breaks long lines instead of scrolling them, for commands a reader copies whole. */
export function CodeBlock({ code, lang = "sh", title, wrap = false }: { code: string; lang?: string; title?: string; wrap?: boolean }) {
  const tokens = highlight(code, lang);
  const label = languageLabel(lang);
  return (
    <figure className={wrap ? `${styles.block} ${styles.wrap}` : styles.block} data-sq="clip" data-docs-code="">
      <figcaption className={styles.header}>
        <span className={styles.label}>
          {title ? <span className={styles.title}>{title}</span> : null}
          <span className={title ? styles.language : styles.languageOnly}>{label}</span>
        </span>
        <CopyCode label={title ? `Copy ${title}` : `Copy ${label} code`} />
      </figcaption>
      <pre className={styles.pre} tabIndex={0} aria-label={title ?? `${label} code`} data-lang={lang || undefined}>
        <code>
          {tokens.map((token, index) => (token.k ? <span key={index} className={styles[token.k]}>{token.v}</span> : token.v))}
        </code>
      </pre>
    </figure>
  );
}
