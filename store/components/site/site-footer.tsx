/**
 * The store's footer (server-rendered), one quiet line as on Silicon Writings: the copyright in the pixel face, a few
 * links (making an app, what a Silicon can read here, the code and the service status), and the theme choice (System,
 * Light, Dark).
 */
import { LINKS } from "@/lib/site";
import { ThemePicker } from "./theme-controls";
import styles from "./site.module.css";

const LINKS_ROW: Array<{ href: string; label: string; external?: boolean }> = [
  { href: LINKS.appsDocs, label: "Make an app", external: true },
  { href: LINKS.accountsDocs, label: "Add sign-in", external: true },
  { href: "/llms.txt", label: "llms.txt" },
  { href: "/openapi.json", label: "API" },
  { href: "/settings", label: "Settings" },
  { href: LINKS.status, label: "Status", external: true },
  { href: LINKS.appsGithub, label: "GitHub", external: true },
];

export function SiteFooter() {
  return (
    <footer className={styles.footer}>
      <div className={styles.footerInner}>
        <p className={styles.copyright}>© {new Date().getFullYear()} Team of Silicons</p>
        <nav aria-label="Footer">
          <ul role="list" className={styles.footerLinks}>
            {LINKS_ROW.map(link => (
              <li key={link.href}>
                <a href={link.href} className={styles.footerLink} target={link.external ? "_blank" : undefined} rel={link.external ? "noopener noreferrer" : undefined}>{link.label}</a>
              </li>
            ))}
          </ul>
        </nav>
        <ThemePicker />
      </div>
    </footer>
  );
}
