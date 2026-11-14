/**
 * The store's frame, on the account site's shell (silicon-accounts web, components/foundation/shell, branch compact):
 * from 900px, the sidebar beside the page, the wordmark at its head (Anil, 2026-11-14: with search and the profile in
 * the sidebar, the top bar went, so the page, a white box that scrolls on its own, runs the window's full height);
 * under 900px, a top bar with the wordmark and the menu button over the full-bleed page.
 *
 * The sidebar is the account site's (sidebar.tsx there) in its inset form (Anil, 2026-11-14): search at its head, the
 * section list (Silicon UI's NavList: Home and All Apps, then the categories under a small label), then at its foot the profile, full width (the signed-in account, which opens
 * Settings, or Sign In), over a hairline and the other sites as small grey chips. Below 900px the same sidebar opens from a menu button in the top bar, as
 * a native popover sheet where the account site uses Silicon UI's BottomSheet, so it works without script.
 * Server-rendered; links are plain links, so every page is a full HTML document.
 */
import type { ReactNode } from "react";
import { House, LayoutGrid, Menu, X } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { NavItem, NavList } from "@teamofsilicons/silicon-ui/nav-list";
import { CategoryIcon } from "@/components/parts/category-icon";
import { BrandGlyph } from "@/components/site/brand";
import { CATEGORIES } from "@/lib/categories";
import { safeImage } from "@/lib/format";
import { signInHref } from "@/lib/session";
import { LINKS } from "@/lib/site";
import type { Account } from "@/lib/types";
import { StoreLinks } from "./links";
import { SearchInput } from "./search-field";
import styles from "./shell.module.css";
import sidebar from "./sidebar.module.css";

const ICON = { size: 16, strokeWidth: 2, "aria-hidden": true } as const;

/** What the visitor searched for, when the page is a search: the field keeps it. */
function queryOf(path: string): string {
  const [pathname, query = ""] = path.split("?");
  return pathname === "/" ? (new URLSearchParams(query).get("q") ?? "").slice(0, 200) : "";
}

function Brand() {
  return (
    <a href="/" data-sq="surface" className={styles.brand} aria-label="Team of Silicons Apps, home">
      <span className={styles.brandMark} aria-hidden="true"><BrandGlyph /></span>
      {/* The account site's wordmark (Team of Silicons Accounts), with this product's name. */}
      <span className={styles.brandText} aria-hidden="true">
        Team of Silicons <b className={styles.brandProduct}>Apps</b>
      </span>
    </a>
  );
}

/** The sidebar: search, the sections, then the profile over the other sites. `id` keeps the two copies' fields apart. */
function Sidebar({ path, account, id, brand = false }: { path: string; account: Account | null; id: string; brand?: boolean }) {
  const pathname = path.split("?")[0];
  return (
    <div className={sidebar.sidebar}>
      {/* Beside the page the sidebar carries the wordmark (the top bar is gone there), as the account site's inset
          shell; the phone menu has the top bar's. */}
      {brand ? <div className={sidebar.brandRow}><Brand /></div> : null}
      {/* Search: a plain GET form to the home page, so Enter searches with or without script. Its label is read, not
          shown. */}
      <form className={sidebar.head} method="get" action="/" role="search" aria-label="Search apps">
        <SearchInput id={`${id}-q`} defaultValue={queryOf(path)} />
      </form>
      <NavList label="Store sections">
        <NavItem href="/" icon={<House {...ICON} />} active={pathname === "/"}>Home</NavItem>
        <NavItem href="/apps" icon={<LayoutGrid {...ICON} />} active={pathname === "/apps"}>All Apps</NavItem>
      </NavList>
      {/* The categories (lib/categories.ts), each its own page; the one open is marked. */}
      <div className={sidebar.group}>
        <span className={sidebar.groupLabel}>Categories</span>
        <NavList label="Categories">
          {CATEGORIES.map(category => (
            <NavItem key={category.id} href={`/categories/${category.id}`} icon={<CategoryIcon id={category.id} />} active={pathname === `/categories/${category.id}`}>{category.title}</NavItem>
          ))}
        </NavList>
      </div>
      <div className={sidebar.account}>
        {account ? (
          <ButtonLink href="/settings" variant="ghost" size="sm" className={`${sidebar.wide} ${sidebar.profile}`} aria-label={`Settings, signed in as ${account.id}`}>
            <Avatar name={account.display_name || account.id} src={safeImage(account.pfp_url)} size="sm" aria-hidden="true" />
            <span className={sidebar.profileId}>{account.id}</span>
          </ButtonLink>
        ) : (
          <ButtonLink href={signInHref(path)} variant="secondary" size="sm" rel="nofollow" className={sidebar.wide}>Sign In</ButtonLink>
        )}
      </div>
      <nav className={sidebar.foot} aria-label="Other sites">
        <ButtonLink href={LINKS.developers} variant="soft" size="xs" target="_blank" rel="noopener noreferrer">Developers</ButtonLink>
        <ButtonLink href={LINKS.appsDocs} variant="soft" size="xs" target="_blank" rel="noopener noreferrer">Docs</ButtonLink>
      </nav>
    </div>
  );
}

export function Shell({ path, account, children }: { path: string; account: Account | null; children: ReactNode }) {
  return (
    <StoreLinks>
      <div className={styles.shell}>
        <a className="skip-link" data-sq="surface" href="#main">Skip to content</a>
        <header className={styles.top}>
          <Brand />
          <span className={styles.menuButton}>
            <Button variant="ghost" size="sm" popoverTarget="site-menu" aria-label="Open the menu">
              <Menu size={18} strokeWidth={2} aria-hidden="true" />
            </Button>
          </span>
        </header>
        <div className={styles.body}>
          <aside className={styles.aside} aria-label="Sidebar"><Sidebar path={path} account={account} id="side" brand /></aside>
          <main id="main" data-sq-native="" className={styles.main} tabIndex={-1}>
            {children}
          </main>
        </div>
        <div id="site-menu" popover="auto" className={styles.menu} data-sq="surface" role="dialog" aria-label="Menu">
          <div className={styles.menuHead}>
            <span className={styles.menuTitle}>Menu</span>
            <Button variant="ghost" size="sm" popoverTarget="site-menu" popoverTargetAction="hide" aria-label="Close the menu">
              <X {...ICON} />
            </Button>
          </div>
          <Sidebar path={path} account={account} id="menu" />
        </div>
      </div>
    </StoreLinks>
  );
}
