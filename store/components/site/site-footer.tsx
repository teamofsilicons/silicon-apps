/**
 * The store's footer (server-rendered), one quiet line as on Silicon Writings: the copyright in the pixel face and a
 * few links (making an app, what a Silicon can read here, the code and the service status).
 */
import { LINKS } from "@/lib/site";
import styles from "./site.module.css";

const LINKS_ROW: Array<{ href: string; label: string; external?: boolean }> = [
  { href: LINKS.appsDocs, label: "Make a Silicon App", external: true },
  { href: LINKS.accountsDocs, label: "Add Silicon Accounts to your app", external: true },
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
      </div>
    </footer>
  );
}
