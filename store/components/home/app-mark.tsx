/**
 * An app's mark: its logo at 32px in a row (any `size` elsewhere: 40 on its page) with the account site's app-mark corner, or its first letter on a quiet
 * tile when it has no logo. Decorative: the row's title names the app.
 */
import { safeImage } from "@/lib/format";
import type { App } from "@/lib/types";
import styles from "./app-mark.module.css";

export function AppMark({ app, size = 32 }: { app: Pick<App, "name" | "logo">; size?: number }) {
  const logo = safeImage(app.logo);
  if (logo) {
    // eslint-disable-next-line @next/next/no-img-element -- the authors' own logo, shown as it is.
    return <img className={styles.mark} data-sq="clip" style={{ width: size, height: size }} src={logo} alt="" width={size} height={size} loading="lazy" decoding="async" />;
  }
  return <span className={styles.mark} data-sq="clip" style={{ width: size, height: size }} aria-hidden="true">{app.name.slice(0, 1)}</span>;
}
