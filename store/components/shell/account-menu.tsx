/**
 * The signed-in account at the foot of the sidebar (Anil, 2026-11-19: what Settings held, "in the sidebar itself in
 * the accounts drop down"): the picture and id as one quiet button, opening a menu above it, a native popover, so it
 * works without script: who you are (picture, name, "c:ada · Carbon"), Manage your account (Silicon Accounts, in a new
 * tab), then Sign Out (a form post to /sign-out that comes back to this page). Anchored to the button where the
 * browser can (CSS anchor positioning); else centred, as a popover is by default. Server-rendered.
 */
import type { CSSProperties } from "react";
import { ArrowUpRight, ChevronsUpDown, LogOut } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { safeImage } from "@/lib/format";
import { LINKS } from "@/lib/site";
import type { Account } from "@/lib/types";
import styles from "./account-menu.module.css";

export function AccountMenu({ account, path, id }: { account: Account; path: string; id: string }) {
  const name = account.display_name || account.id;
  const menu = `${id}-account`;
  const anchor = { "--anchor": `--${menu}` } as CSSProperties;
  return (
    <>
      <Button variant="ghost" size="sm" className={styles.trigger} popoverTarget={menu} aria-haspopup="menu" aria-label={`Account, signed in as ${account.id}`} style={anchor}>
        <Avatar name={name} src={safeImage(account.pfp_url)} size="sm" aria-hidden="true" />
        <span className={styles.id}>{account.id}</span>
        <ChevronsUpDown size={14} strokeWidth={2} aria-hidden="true" className={styles.chevron} />
      </Button>
      <div id={menu} popover="auto" className={styles.menu} style={anchor} role="dialog" aria-label="Account">
        <div className={styles.who}>
          <Avatar name={name} src={safeImage(account.pfp_url)} size="md" aria-hidden="true" />
          <span className={styles.words}>
            <span className={styles.name}>{name}</span>
            <span className={styles.detail}>{account.id} · {account.id.startsWith("si:") ? "Silicon" : "Carbon"}</span>
          </span>
        </div>
        <a href={LINKS.accounts} target="_blank" rel="noopener noreferrer" className={styles.item}>
          <span>Manage your account</span>
          <ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" />
        </a>
        <form method="post" action="/sign-out" className={styles.signOut}>
          {/* eslint-disable-next-line no-restricted-syntax -- /sign-out is a route handler, not a server action, so where to come back is a plain field */}
          <input type="hidden" name="return_to" value={path} />
          <Button type="submit" variant="ghost" size="sm" className={styles.item}>
            <span>Sign Out</span>
            <LogOut size={14} strokeWidth={2} aria-hidden="true" />
          </Button>
        </form>
      </div>
    </>
  );
}
