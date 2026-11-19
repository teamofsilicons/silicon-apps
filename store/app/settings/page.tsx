/**
 * /settings: retired (Anil, 2026-11-19). What it held moved to the sidebar: the account and Sign Out to the account
 * menu (components/shell/account-menu.tsx), Report a problem to the sidebar's foot (components/shell/report.tsx). Old
 * links land on the home page.
 */
import { permanentRedirect } from "next/navigation";

export default function Settings() {
  permanentRedirect("/");
}
