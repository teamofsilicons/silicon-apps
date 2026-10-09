/**
 * The store's footer (server-rendered), as on the developer site (developer/components/site/site-footer.tsx): the
 * store, where to build, everything a Silicon can read or call here, the rest of the ecosystem (service status and the
 * open source code on GitHub), and the theme choice (System, Light, Dark).
 */
import { ArrowUpRight } from "lucide-react";
import { LINKS } from "@/lib/site";
import { BrandMark } from "./brand";
import { ThemePicker } from "./theme-controls";
import styles from "./site.module.css";

const COLUMNS: Array<{ title: string; links: Array<{ href: string; label: string; external?: boolean }> }> = [
  {
    title: "Store",
    links: [
      { href: "/", label: "Discover apps" },
      { href: "/search", label: "All apps" },
      { href: "/search?visibility=private", label: "Private apps" },
      { href: "/#get-the-cli", label: "Get the CLI" },
      { href: "/settings", label: "Settings" },
    ],
  },
  {
    title: "Build",
    links: [
      { href: LINKS.appsDocs, label: "Make an app", external: true },
      { href: LINKS.accountsDocs, label: "Add sign-in", external: true },
      { href: LINKS.developers, label: "Silicon Developer", external: true },
    ],
  },
  {
    title: "For Silicons",
    links: [
      { href: "/llms.txt", label: "llms.txt" },
      { href: "/llms-full.txt", label: "llms-full.txt" },
      { href: "/#mcp", label: "MCP server" },
      { href: "/openapi.json", label: "OpenAPI" },
      { href: "/.well-known/agent.json", label: "Agent card" },
    ],
  },
  {
    title: "Ecosystem",
    links: [
      { href: LINKS.accounts, label: "Silicon Accounts", external: true },
      { href: LINKS.teamOfSilicons, label: "Team of Silicons", external: true },
      { href: LINKS.status, label: "Status", external: true },
      { href: LINKS.appsGithub, label: "Apps on GitHub", external: true },
    ],
  },
];

export function SiteFooter() {
  return (
    <footer className={styles.footer}>
      <div className={styles.footerInner}>
        <div className={styles.footerTop}>
          <div className={styles.footerIntro}>
            <a href="/" className={styles.footerBrand}>
              <BrandMark className={styles.brandMark} />
              <span>Silicon Apps</span>
            </a>
            <p className={styles.tagline}>
              The app store of the Silicon ecosystem. Every app is made for Carbons and Silicons, installs with one command
              and keeps itself up to date.
            </p>
          </div>
          <nav className={`${styles.columns} ${styles.columnsFour}`} aria-label="Footer">
            {COLUMNS.map(column => (
              <section key={column.title} className={styles.column} aria-labelledby={`footer-${column.title.toLowerCase().replace(/\s+/g, "-")}`}>
                <h2 id={`footer-${column.title.toLowerCase().replace(/\s+/g, "-")}`}>{column.title}</h2>
                <ul role="list">
                  {column.links.map(link => (
                    <li key={link.href}>
                      <a href={link.href} className={styles.footerLink} rel={link.external ? "noopener" : undefined}>
                        {link.label}
                        {link.external ? <ArrowUpRight size={13} strokeWidth={1.75} aria-hidden="true" className={styles.externalIcon} /> : null}
                      </a>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </nav>
        </div>
        <div className={styles.footerBottom}>
          <p className={styles.copyright}>
            © {new Date().getFullYear()} Team of Silicons. Silicon Apps is open source under the MIT license, so we have
            nothing to hide: <a href={LINKS.appsGithub} rel="noopener">read the code on GitHub</a>.
          </p>
          <ThemePicker />
        </div>
      </div>
    </footer>
  );
}
