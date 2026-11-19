/**
 * An app page's header, Editorial (Anil, 2026-11-18: picked from the lab at /playground/app-header, where B · Cover is
 * kept beside it). No box and no ground:
 * - the icon (72px, 56 on a phone) on the left and beside it the name (display) and the tagline as a lede (h2's size,
 *   regular), else the description's first sentence; on the right of the same row (under it when narrow) the actions:
 *   Open Website where the app has one, the install command with a copy button, Manage App for its authors, the note;
 * - a hairline, then the labelled facts in a row: Developer (each a chip: a picture and the name, a link to their page;
 *   the lab compares other ways), Category (a chip), Version, Systems (each a chip: its mark and name), Signed (who
 *   signed the current release, a link to how signing works; left out until there is a release). Rating and installs
 *   are left out while most apps have none (Anil, 2026-11-18).
 * The earlier header (the icon in a column, the words and the actions beside it) is in git. Server-rendered.
 */
import { ShieldCheck } from "lucide-react";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { Actions, CategoryChip, Developers, headerFacts, Note, type DeveloperStyle } from "@/components/app/header-lab/parts";
import { AppMark } from "@/components/home/app-mark";
import { OsMark } from "@/components/home/os-icons";
import { withAppIcon } from "@/lib/app-icons";
import { operatingSystems } from "@/lib/format";
import { LINKS } from "@/lib/site";
import type { App } from "@/lib/types";
import styles from "./app-header.module.css";

/** The labelled facts: a row across the page under the header, or `stacked` one under another (a sidebar);
 * `withDevelopers` false leaves the developers out where a developer card already shows them. */
export function AppFacts({ app, developers = "chips", stacked = false, withDevelopers = true }: { app: App; developers?: DeveloperStyle; stacked?: boolean; withDevelopers?: boolean }) {
  const facts = headerFacts(app);
  const systems = operatingSystems(app.targets);
  const release = app.latest_production ?? app.latest_development;
  return (
    <dl className={styles.list} data-stacked={stacked ? "" : undefined}>
      {withDevelopers && app.authors.length ? <div><dt>{app.authors.length === 1 ? "Developer" : "Developers"}</dt><dd><Developers app={app} style={developers} /></dd></div> : null}
      {facts.category ? <div><dt>Category</dt><dd><CategoryChip facts={facts} /></dd></div> : null}
      <div><dt>Version</dt><dd className={styles.version}>{facts.version ?? "None yet"}</dd></div>
      <div><dt>Systems</dt><dd>{systems.length ? (
        <ul className={styles.chips}>
          {systems.map(system => <li key={system} className={styles.chip}><OsMark system={system} className={styles.mark} />{system}</li>)}
        </ul>
      ) : <span className={styles.quiet}>None yet</span>}</dd></div>
      {release ? (
        <div>
          <dt>{app.signed ? "Signed by" : "Signed"}</dt>
          <dd>
            <a href={`${LINKS.appsDocs}/learn/signed-releases`} target="_blank" rel="noopener noreferrer" className={styles.signed} data-signed={app.signed ? "" : undefined} title={app.signed ? `Signed by Silicon Apps${app.signed_by?.length ? ` and by ${app.signed_by.join(", ")}` : ""}` : undefined}>
              {app.signed ? <><ShieldCheck size={16} strokeWidth={2} aria-hidden="true" />{app.signed_by_author ? "Silicon Apps and developer" : "Silicon Apps"}</> : "Not signed yet"}
            </a>
          </dd>
        </div>
      ) : null}
    </dl>
  );
}

/** The actions and the note under them (Open Website, the install command, Manage App), for a header or a sidebar. */
export function AppGet({ app, stack = false }: { app: App; stack?: boolean }) {
  const facts = headerFacts(app);
  return (
    <div className={styles.get}>
      <Actions app={app} facts={facts} stack={stack} />
      <Note facts={facts} />
    </div>
  );
}

/** `compact` keeps only the icon, the name and the lede (the actions and facts go elsewhere, as in the page-layout
 * lab's sidebar). */
export function AppHeader({ app, developers = "chips", compact = false }: { app: App; developers?: DeveloperStyle; compact?: boolean }) {
  const facts = headerFacts(app);
  return (
    <header className={styles.header}>
      <div className={styles.top}>
        <span className={styles.icon}><AppMark app={withAppIcon(app)} size={72} /></span>
        <div className={styles.words}>
          <h1 id="app-name" className={styles.name}>
            {app.name}
            {app.visibility === "private" ? <Badge size="sm" tone="neutral">Private</Badge> : null}
          </h1>
          {facts.line ? <p className={styles.line}>{facts.line}</p> : null}
        </div>
        {compact ? null : <AppGet app={app} />}
      </div>
      {compact ? null : <div className={styles.facts}><AppFacts app={app} developers={developers} /></div>}
    </header>
  );
}
