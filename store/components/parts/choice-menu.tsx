"use client";
/**
 * A dropdown of links (Anil, 2026-11-14, for a category's choices): Silicon UI's secondary Button showing the choice's
 * label (unless the option says it alone, as "All platforms" does) and its current option, opening a native popover (anchored under the button where the browser can) whose
 * options are links, the current one checked. The popover is the browser's own, so it opens and works without script;
 * this is a client component only so the Button's children are the same elements on the server and in the browser.
 */
import type { CSSProperties } from "react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { Check, ChevronDown } from "lucide-react";
import styles from "./choice-menu.module.css";

export type ChoiceOption = { label: string; href: string; current: boolean };

export function ChoiceMenu({ id, label, showLabel = true, options }: { id: string; label: string; showLabel?: boolean; options: ChoiceOption[] }) {
  const anchor = { "--anchor": `--${id}` } as CSSProperties;
  return (
    <div className={styles.choice}>
      <Button variant="secondary" size="sm" className={styles.trigger} popoverTarget={id} aria-haspopup="true" aria-label={showLabel ? undefined : `${label}: ${options.find(option => option.current)?.label}`} style={anchor}>
        {showLabel ? <span className={styles.label}>{label}</span> : null}
        <span>{options.find(option => option.current)?.label}</span>
        <ChevronDown size={14} aria-hidden="true" className={styles.chevron} />
      </Button>
      <nav id={id} popover="auto" className={styles.menu} style={anchor} aria-label={label}>
        {options.map(option => (
          <a key={option.href} href={option.href} className={styles.item} aria-current={option.current ? "page" : undefined}>
            <span>{option.label}</span>
            {option.current ? <Check size={14} aria-hidden="true" /> : null}
          </a>
        ))}
      </nav>
    </div>
  );
}
