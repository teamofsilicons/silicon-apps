/**
 * The home page's banner (Anil, 2026-11-14): what every app here shares, that one command installs it, as a single
 * quiet terminal window on one of the account site's soft gradients (public/banner/bg-102.webp). The window shows the
 * store's own example: installing Briefcase, then asking it for help (every app answers --help). Decorative, since
 * "Install an App" below says the same in words, so it is hidden from screen readers. Server-rendered, no script.
 */
import styles from "./hero.module.css";

const LINES: { kind: "command" | "output"; text: string }[] = [
  { kind: "command", text: "silicon-apps install briefcase" },
  { kind: "output", text: "Installed briefcase." },
  { kind: "command", text: "briefcase --help" },
];

export function Hero() {
  return (
    <div className={styles.hero} data-sq="clip" aria-hidden="true">
      <div className={styles.window} data-sq="surface">
        <div className={styles.bar}>
          <span className={styles.dot} />
          <span className={styles.dot} />
          <span className={styles.dot} />
        </div>
        <div className={styles.screen}>
          {LINES.map((line, index) => (
            <p key={index} className={styles.line} data-kind={line.kind}>
              {line.kind === "command" ? <span className={styles.prompt}>$ </span> : null}
              {line.text}
            </p>
          ))}
          <p className={styles.line} data-kind="command"><span className={styles.prompt}>$ </span><span className={styles.cursor} /></p>
        </div>
      </div>
    </div>
  );
}
