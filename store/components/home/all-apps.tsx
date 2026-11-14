/**
 * Apps as the account site's Connected Apps rows (Anil, 2026-11-14), for search results and a category's list view: the app's logo, its name, its first sentence, then a third line with its
 * rating (the number and a star) and the marks of the systems it runs on, the logo and words aligned to the top; the
 * whole row a link to its page. Server-rendered. (The home page's All Apps list that first used them came off, 2026-11-14.)
 */
import { Search } from "lucide-react";
import { EmptyState } from "@teamofsilicons/silicon-ui/empty-state";
import { LinkItem, List } from "@/components/parts/rows";
import { formatRating } from "@/lib/format";
import { withAppIcon } from "@/lib/app-icons";
import type { App } from "@/lib/types";
import styles from "./all-apps.module.css";
import { AppMark } from "./app-mark";
import { limitWords, withoutName } from "./feature";
import { OsIcons } from "./os-icons";

/** A row's third line: the rating as the number and a star (the list's earlier design), then the marks of the systems
 * it runs on. */
function Meta({ app }: { app: App }) {
  return (
    <>
      {app.rating !== null ? <span aria-label={`${formatRating(app.rating)} out of 5 stars`}>{formatRating(app.rating)} <span className={styles.star} aria-hidden="true">★</span></span> : <span>No ratings yet</span>}
      <OsIcons targets={app.targets} />
    </>
  );
}

/** The apps as the account site's rows (used by the home list, search results and a category's list view). */
export function AppRows({ apps, label }: { apps: App[]; label: string }) {
  if (!apps.length) {
    return (
      <div data-sq="surface" className={styles.empty}>
        <EmptyState icon={<Search size={16} strokeWidth={2} />} title="No apps here yet" description="Apps show up here the moment their authors publish them." />
      </div>
    );
  }
  return (
    <List label={label}>
      {apps.map(app => (
        <LinkItem
          key={app.app_id}
          href={`/apps/${encodeURIComponent(app.app_id)}`}
          lead={<AppMark app={withAppIcon(app)} />}
          title={app.name}
          detail={app.description ? limitWords(withoutName(app.name, app.description), 15) : "No description yet."}
          meta={<Meta app={app} />}
        />
      ))}
    </List>
  );
}
