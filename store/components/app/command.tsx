/**
 * The command that installs an app, as one line of mono text with Silicon UI's copy button at its end. The button
 * needs script, so without it the line is plain, selectable text. Server-rendered around the button.
 */
import { CopyButton } from "@teamofsilicons/silicon-ui/copy-button";
import styles from "./command.module.css";

export function Command({ value, label }: { value: string; label: string }) {
  return (
    <div className={styles.command} data-sq="surface">
      <code className={styles.text}><span className={styles.prompt} aria-hidden="true">$ </span>{value}</code>
      <span className={styles.copy} data-js-only="">
        <CopyButton value={value} label={label} iconOnly variant="plain" />
      </span>
    </div>
  );
}
