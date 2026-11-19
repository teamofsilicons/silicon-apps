/**
 * An app page's Developer (Anil, 2026-11-18), after Compatibility: who made it, where to find more, and what else they made.
 * Profile (Anil, 2026-11-18: picked from the lab at /playground/developer; after Google Play's About the developer and
 * GitHub's sidebar), two columns (one on a narrow screen):
 * - a card for who made it and where to find them: each author linked to their page (their picture, their name over
 *   their id and how many apps they have published), then under a hairline the links: the app's website, developer
 *   docs, store pages and the authors' own (Source, API…), each its icon, its label over its address, in a new tab
 *   (http(s) only). The card's ground is CARD_STYLES (glow on the page).
 * - beside it, More by the developer(s): their other apps as a compact list in columns (a 36px icon, the name over one
 *   line), every author's together, this app and repeats left out, at most six; "See all" goes to the author's page
 *   where there is one author.
 * The authors' apps are GET /v1/authors/{uuid} (lib/developers.ts). Server-rendered.
 */
import { BookOpen, CodeXml, Globe, Link as LinkIcon, Smartphone, type LucideIcon } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { firstSentence } from "@/components/gallery/gallery";
import { AppMark } from "@/components/home/app-mark";
import { withAppIcon } from "@/lib/app-icons";
import type { DeveloperWork } from "@/lib/developers";
import { safeImage, safeUrl } from "@/lib/format";
import { appPath, authorPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./developer.module.css";

export type AppLink = { label: string; url: string; host: string; icon: LucideIcon };

/** The app's links, website first, then docs, the store pages, then the authors' own (at most six), each http(s) only. */
export function linksOf(app: App): AppLink[] {
  const raw = app.links ?? {};
  const custom = Array.isArray(raw.custom) ? raw.custom.slice(0, 6) : [];
  const list = [
    { label: "Website", url: raw.website, icon: Globe },
    { label: "Developer docs", url: raw.developer_docs ?? raw.docs, icon: BookOpen },
    { label: "Android app", url: raw.android, icon: Smartphone },
    { label: "iOS app", url: raw.ios, icon: Smartphone },
    ...custom.map(link => ({ label: link.label?.trim() || "Link", url: link.url, icon: /^source( code)?$/i.test(link.label ?? "") ? CodeXml : LinkIcon })),
  ];
  return list.flatMap(link => {
    const url = safeUrl(link.url);
    if (!url || !/^https?:/.test(url)) return [];
    const { host, pathname } = new URL(url);
    const path = pathname.replace(/\/$/, "");
    return [{ label: link.label, url, icon: link.icon, host: host.replace(/^www\./, "") + (path && host === "github.com" ? path : "") }];
  });
}

/** The organizations behind the app (main's organization publishing): the publisher first, then the other author
 * organizations, each once. They have no page in the store yet, so they aren't links. */
export function organizationsOf(app: App): { id: string; name: string; logo?: string | null; publisher: boolean }[] {
  const out: { id: string; name: string; logo?: string | null; publisher: boolean }[] = [];
  const publisher = app.publisher?.type === "organization" ? app.publisher : undefined;
  if (publisher) out.push({ id: publisher.id || publisher.name || "organization", name: publisher.name || publisher.id || "Organization", logo: publisher.logo_url, publisher: true });
  for (const org of app.author_organizations ?? []) if (!out.some(o => o.id === org)) out.push({ id: org, name: org, publisher: false });
  return out;
}
export const nameOf = (author: App["authors"][number]) => author.display_name || author.id;
export const appsCount = (count: number) => `${count} ${count === 1 ? "app" : "apps"}`;

/** How the developer card is grounded, compared in the lab (?card=; Anil, 2026-11-18: Profile picked, "need the
 * developer profile card to have a better background color"):
 *   raised  the raised surface (white in light) with a hairline and a soft shadow, lifted off the page
 *   outline no fill, a hairline round it
 *   tint    the accent's soft wash
 *   glow    the accent's wash fading into the raised surface, top left to bottom right, with a hairline (the page's,
 *           Anil, 2026-11-18, after raised read as a card floating inside the page's own panel) */
export const CARD_STYLES = ["raised", "outline", "tint", "glow"] as const;
export type CardStyle = (typeof CARD_STYLES)[number];

/** Who made it and where to find them: the people, then under a hairline the links, on the card's ground. */
export function DeveloperCard({ app, work, card = "glow" }: { app: App; work: DeveloperWork; card?: CardStyle }) {
  const links = linksOf(app);
  const orgs = organizationsOf(app);
  if (!app.authors.length && !orgs.length && !links.length) return null;
  return (
    <div className={styles.card} data-card={card}>
      <ul className={styles.cardPeople}>
        {orgs.map(org => (
          <li key={`org:${org.id}`}>
            <span className={styles.cardPerson}>
              <Avatar name={org.name} src={safeImage(org.logo)} size="md" aria-hidden="true" />
              <span className={styles.who}>
                <span className={styles.whoName}>{org.name}</span>
                <span className={styles.whoId}>{org.publisher ? "Organization · publisher" : "Organization"}</span>
              </span>
            </span>
          </li>
        ))}
        {app.authors.map(author => (
          <li key={author.uuid}>
            <a href={authorPath(author.uuid)} className={styles.cardPerson}>
              <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size="md" aria-hidden="true" />
              <span className={styles.who}>
                <span className={styles.whoName}>{nameOf(author)}</span>
                <span className={styles.whoId}>{author.id}{work.totals[author.uuid] ? ` · ${appsCount(work.totals[author.uuid])}` : ""}</span>
              </span>
            </a>
          </li>
        ))}
      </ul>
      {links.length ? (
        <ul className={styles.cardLinks} aria-label="Links">
          {links.map((link, index) => {
            const Icon = link.icon;
            return (
              <li key={index}>
                <a href={link.url} className={styles.cardLink} target="_blank" rel="nofollow ugc noopener noreferrer">
                  <Icon size={16} strokeWidth={2} aria-hidden="true" className={styles.cardIcon} />
                  <span className={styles.cardLabel}>{link.label}</span>
                  <span className={styles.cardHost}>{link.host}</span>
                </a>
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}

/** More by the developer(s): a compact list in columns (one where the room is narrow), See all for one author. */
export function MoreBy({ app, work }: { app: App; work: DeveloperWork }) {
  if (!work.more.length) return null;
  const more = `More by ${app.authors.length === 1 ? nameOf(app.authors[0]) : "these developers"}`;
  return (
    <div className={styles.group}>
      <div className={styles.head}><h3 className={styles.subtitle}>{more}</h3>{app.authors.length === 1 ? <a href={authorPath(app.authors[0].uuid)} className={styles.all}>See all</a> : null}</div>
      <ul className={styles.compact} aria-label={more}>
        {work.more.map(item => (
          <li key={item.app_id}>
            <a href={appPath(item.app_id)} className={styles.compactRow}>
              <AppMark app={withAppIcon(item)} size={36} />
              <span className={styles.who}>
                <span className={styles.whoName}>{item.name}</span>
                <span className={styles.whoId}>{item.tagline?.trim() || firstSentence(item) || "No description yet."}</span>
              </span>
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function Developer({ app, work, card = "glow" }: { app: App; work: DeveloperWork; card?: CardStyle }) {
  if (!app.authors.length && !organizationsOf(app).length && !linksOf(app).length) return null;
  return (
    <section className={styles.section} aria-labelledby="developer-title">
      <h2 id="developer-title" className={styles.title}>{app.authors.length > 1 ? "Developers" : "Developer"}</h2>
      <div className={styles.split}>
        <DeveloperCard app={app} work={work} card={card} />
        <MoreBy app={app} work={work} />
      </div>
    </section>
  );
}
