/**
 * The pieces every header design in the lab (/playground/app-header, Anil, 2026-11-18) is built from, so the designs
 * differ in layout and ground, not in content: what the header says (headerFacts), "By" the authors, the category chip,
 * the actions (Open Website, the install command, Manage App) and the note under them. Server-rendered.
 */
import { ArrowUpRight } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { AvatarGroup } from "@teamofsilicons/silicon-ui/avatar-group";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { Command } from "@/components/app/command";
import { firstSentence } from "@/components/gallery/gallery";
import { CategoryIcon } from "@/components/parts/category-icon";
import { APP_PICTURES } from "@/lib/app-pictures";
import { categoryOf } from "@/lib/categories";
import { safeImage, safeUrl } from "@/lib/format";
import { authorPath } from "@/lib/seo";
import { installCommand, LINKS, manageAppUrl } from "@/lib/site";
import type { App } from "@/lib/types";
import styles from "./parts.module.css";

export type HeaderFacts = {
  line: string;
  website: string | null;
  command: string;
  note: string;
  category: ReturnType<typeof categoryOf>;
  version: string | null;
  /** The app's own picture, else its banner, else its first picture; null when it has none. */
  cover: { src: string; focus?: string } | null;
};

export function headerFacts(app: App): HeaderFacts {
  const site = safeUrl(app.links?.website);
  const devOnly = !app.latest_production && Boolean(app.latest_development);
  const release = app.latest_production ?? app.latest_development;
  const banner = safeImage(app.banner);
  const first = (app.carousel ?? []).find(item => item.kind !== "video" && safeImage(item.url));
  return {
    line: app.tagline?.trim() || firstSentence(app),
    website: site && /^https?:/.test(site) ? site : null,
    command: devOnly ? `silicon-apps install '${app.app_id}>dev'` : installCommand(app.app_id),
    note: !release
      ? "There is no release to install yet."
      : devOnly
        ? "Only development releases so far, so this installs the latest one."
        : "Installs the latest release for your system and keeps it up to date.",
    category: categoryOf(app.app_id),
    version: release?.version ?? null,
    cover: APP_PICTURES[app.app_id] ?? (banner ? { src: banner } : first ? { src: safeImage(first.url)! } : null),
  };
}

const nameOf = (author: App["authors"][number]) => author.display_name || author.id;

/** "By" and each author, a small picture and a name, linked to their page. */
export function Authors({ app, by = true }: { app: App; by?: boolean }) {
  if (!app.authors.length) return null;
  return (
    <span className={styles.authors}>
      {by ? "By " : null}
      {app.authors.map((author, index) => (
        <span key={author.uuid} className={styles.author}>
          <a href={authorPath(author.uuid)} title={author.id}>
            <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size="sm" aria-hidden="true" />
            {nameOf(author)}
          </a>
          {index < app.authors.length - 1 ? "," : null}
        </span>
      ))}
    </span>
  );
}

/** The ways to show who made an app, compared in the lab (?dev=; Anil, 2026-11-18: "made by" read loose, and a comma
 * between names didn't fit):
 *   chips    each developer a pill: a 20px picture and the name (the page's)
 *   profile  each developer a 36px picture, the name over the id
 *   faces    the pictures overlapping, then the first name and how many others
 *   plain    a 20px picture and the name, no box, the names apart by a gap */
export const DEVELOPER_STYLES = ["chips", "profile", "faces", "plain"] as const;
export type DeveloperStyle = (typeof DEVELOPER_STYLES)[number];

export function Developers({ app, style = "chips" }: { app: App; style?: DeveloperStyle }) {
  if (!app.authors.length) return null;
  if (style === "faces") {
    const [first, ...rest] = app.authors;
    return (
      <a href={authorPath(first.uuid)} className={styles.faces}>
        <AvatarGroup members={app.authors.map(author => ({ name: nameOf(author), src: safeImage(author.pfp_url) }))} max={4} size="sm" label="Developers" />
        <span>{nameOf(first)}{rest.length ? <span className={styles.others}> and {rest.length === 1 ? "1 other" : `${rest.length} others`}</span> : null}</span>
      </a>
    );
  }
  return (
    <ul className={styles.developers} data-style={style}>
      {app.authors.map(author => (
        <li key={author.uuid}>
          <a href={authorPath(author.uuid)} className={styles.developer} title={style === "profile" ? undefined : author.id}>
            <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size={style === "profile" ? "md" : "xs"} aria-hidden="true" />
            {style === "profile" ? (
              <span className={styles.who}><span className={styles.devName}>{nameOf(author)}</span><span className={styles.devId}>{author.id}</span></span>
            ) : <span className={styles.devName}>{nameOf(author)}</span>}
          </a>
        </li>
      ))}
    </ul>
  );
}

/** The category as a quiet chip, a link to its page. */
export function CategoryChip({ facts }: { facts: HeaderFacts }) {
  if (!facts.category) return null;
  return (
    <a href={`/categories/${facts.category.id}`} className={styles.category}>
      <CategoryIcon id={facts.category.id} size={14} />{facts.category.title}
    </a>
  );
}

/** Open Website (where there is one), the install command, Manage App for its authors. `stack` puts each on its own line,
 * full width (an install card). */
export function Actions({ app, facts, stack = false }: { app: App; facts: HeaderFacts; stack?: boolean }) {
  return (
    <div className={styles.actions} data-stack={stack ? "" : undefined}>
      {facts.website ? (
        <ButtonLink href={facts.website} target="_blank" rel="nofollow ugc noopener noreferrer">
          Open Website<ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
        </ButtonLink>
      ) : null}
      <div className={styles.command}><Command value={facts.command} label={`Copy the command that installs ${app.name}`} /></div>
      {app.is_author ? (
        <ButtonLink href={manageAppUrl(app.app_id)} variant="secondary" target="_blank" rel="noopener noreferrer">
          Manage App<ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
        </ButtonLink>
      ) : null}
    </div>
  );
}

/** What the command installs, and where to get the CLI. */
export function Note({ facts }: { facts: HeaderFacts }) {
  return (
    <p className={styles.note}>
      {facts.note} New to it? <a href={LINKS.installDocs} target="_blank" rel="noopener noreferrer">Get the CLI</a>.
    </p>
  );
}
