/**
 * The store's header (server-rendered), as on the developer site (developer/components/site/site-header.tsx): the
 * brand, the main navigation, search, the theme switch, and Sign in (or the signed-in account, which opens Settings).
 * Below 900 px the navigation moves into a menu: a native popover, so it opens, closes on Escape or an outside tap,
 * and keeps focus without any script of ours. Links are plain links: every page is a full HTML document.
 */
import { ArrowUpRight, Menu, Search, X } from "lucide-react";
import type { Account } from "@/lib/types";
import { signInHref as signInPath } from "@/lib/session";
import { LINKS } from "@/lib/site";
import { Action } from "./action";
import { BrandMark } from "./brand";
import { ThemeToggle } from "./theme-controls";
import styles from "./site.module.css";

const NAV: Array<{ href: string; label: string; external?: boolean }> = [
  { href: "/search", label: "All apps" },
  { href: "/search?visibility=private", label: "Private apps" },
  { href: "/#for-silicons", label: "For Silicons" },
  { href: LINKS.appsDocs, label: "Make an app", external: true },
];

/** "page" on the link to this very page, nothing on the others (an app page belongs to All apps). */
function currentOf(href: string, path: string): "page" | "true" | undefined {
  const [pathname, query = ""] = path.split("?");
  const visibility = new URLSearchParams(query).get("visibility");
  if (href === "/search") {
    if (pathname === "/search" && visibility !== "private") return query ? "true" : "page";
    return pathname.startsWith("/apps/") || pathname.startsWith("/authors/") ? "true" : undefined;
  }
  if (href === "/search?visibility=private") return pathname === "/search" && visibility === "private" ? "page" : undefined;
  return undefined;
}

export interface SiteHeaderProps {
  /** This page's path and query, to mark the current link and to come back here after signing in. */
  path: string;
  account: Account | null;
}


function initial(account: Account): string {
  const letter = [...(account.display_name || account.id).replace(/^(c|si):/, "")].find(char => /\p{L}|\p{N}/u.test(char));
  return (letter ?? "?").toUpperCase();
}

export function SiteHeader({ path, account }: SiteHeaderProps) {
  return (
    <header className={styles.header}>
      <a className="skip-link" data-sq="surface" href="#main">Skip to content</a>
      <div className={styles.bar}>
        <a href="/" className={styles.brand} data-sq="surface" aria-label="Silicon Apps, home">
          <BrandMark className={styles.brandMark} />
          <span className={styles.brandText} aria-hidden="true">Silicon <span className={styles.brandMuted}>Apps</span></span>
        </a>
        <nav className={styles.nav} aria-label="Main">
          <ul role="list" className={styles.navList}>
            {NAV.map(item => (
              <li key={item.href}>
                <a href={item.href} className={styles.navLink} data-sq="surface" aria-current={currentOf(item.href, path)} rel={item.external ? "noopener" : undefined}>
                  {item.label}
                  {item.external ? <ArrowUpRight size={14} strokeWidth={1.75} aria-hidden="true" className={styles.navExternal} /> : null}
                </a>
              </li>
            ))}
          </ul>
        </nav>
        <div className={styles.actions}>
          <a href="/search" className={styles.iconButton} data-sq="surface" aria-label="Search apps">
            <Search size={17} strokeWidth={1.75} aria-hidden="true" />
          </a>
          <ThemeToggle />
          {account ? (
            <a href="/settings" className={`${styles.accountChip} ${styles.account}`} data-sq="surface" aria-label={`Settings, signed in as ${account.id}`}>
              <span className={styles.accountAvatar} data-sq="surface" aria-hidden="true">{initial(account)}</span>
              <span className={styles.accountId}>{account.id}</span>
            </a>
          ) : (
            <Action href={signInPath(path)} size="sm" className={styles.account} rel="nofollow">Sign in</Action>
          )}
          <button type="button" className={`${styles.iconButton} ${styles.menuButton}`} data-sq="surface" popoverTarget="site-menu" aria-label="Open the menu">
            <Menu size={18} strokeWidth={1.75} aria-hidden="true" />
          </button>
        </div>
      </div>
      <div id="site-menu" popover="auto" className={styles.menu} data-sq="surface" role="dialog" aria-label="Menu">
        <div className={styles.menuHead}>
          <span className={styles.menuTitle}>Menu</span>
          <button type="button" className={styles.iconButton} data-sq="surface" popoverTarget="site-menu" popoverTargetAction="hide" aria-label="Close the menu">
            <X size={18} strokeWidth={1.75} aria-hidden="true" />
          </button>
        </div>
        <nav aria-label="Main menu" className={styles.menuNav}>
          <ul role="list">
            <li><a href="/" className={styles.menuLink} data-sq="surface" aria-current={path.split("?")[0] === "/" ? "page" : undefined}>Home</a></li>
            {NAV.map(item => (
              <li key={item.href}>
                <a href={item.href} className={styles.menuLink} data-sq="surface" aria-current={currentOf(item.href, path)} rel={item.external ? "noopener" : undefined}>
                  {item.label}
                  {item.external ? <ArrowUpRight size={15} strokeWidth={1.75} aria-hidden="true" className={styles.navExternal} /> : null}
                </a>
              </li>
            ))}
            <li><a href="/search" className={styles.menuLink} data-sq="surface">Search apps</a></li>
            <li><a href="/settings" className={styles.menuLink} data-sq="surface" aria-current={path.split("?")[0] === "/settings" ? "page" : undefined}>Settings</a></li>
          </ul>
        </nav>
        <div className={styles.menuFoot}>
          {account ? (
            <Action href="/settings" size="md" variant="secondary" className={styles.menuAction}>Signed in as {account.id}</Action>
          ) : (
            <Action href={signInPath(path)} size="md" className={styles.menuAction} rel="nofollow">Sign in</Action>
          )}
        </div>
      </div>
    </header>
  );
}
