/**
 * Arc's icon-only copy button, as markup (no hooks, so server pages and client islands both render it): the store's
 * script island (components/site/enhancer.tsx) copies data-copy-value, or the block's code, and swaps data-state
 * between idle and copied. Without script it stays hidden ([data-js-only]).
 */
import { Check, Copy } from "lucide-react";
import copyStyles from "@/components/silicon-ui/copy-button/copy-button.module.css";
import styles from "./code-block.module.css";

/** Arc's icon-only copy button, as markup: the island swaps data-state between idle and copied. */
export function CopyCode({ label, value }: { label: string; value?: string }) {
  return (
    <button type="button" className={`${copyStyles.button} ${copyStyles.iconOnly} ${copyStyles.plain} ${styles.copy}`} data-copy="" data-copy-value={value} data-js-only="" data-state="idle" aria-label={label} data-label={label}>
      <span className={styles.copyIcon} aria-hidden="true">
        <Copy size={16} strokeWidth={1.75} data-icon="idle" />
        <Check size={16} strokeWidth={1.75} data-icon="copied" />
      </span>
    </button>
  );
}

