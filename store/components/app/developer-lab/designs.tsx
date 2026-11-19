/**
 * Other designs for an app page's Developer, for the lab at /playground/developer (Anil, 2026-11-18: "what other design
 * can we think of here?", More by included). Each has the same content as the page's (components/app/developer.tsx):
 * who made it, the links, and up to six more apps by them. Server-rendered; plain links.
 *   ShelfDesign    B, after the App Store: the people as chips, the links as pills, More by as a sideways shelf of tiles
 *   GridDesign     A, the page's first: the people as the store's app rows on its grid, More by as the app grid
 *   (C, Profile, became the page's on 2026-11-18: components/app/developer.tsx)
 *   BylineDesign   D, after Raycast's store: one sentence for who, the links as words, More by as a row of icons
 */
import { ArrowUpRight } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { appsCount, linksOf, nameOf } from "@/components/app/developer";
import { Gallery } from "@/components/gallery/gallery";
import { firstSentence } from "@/components/gallery/gallery";
import { AppMark } from "@/components/home/app-mark";
import { withAppIcon } from "@/lib/app-icons";
import type { DeveloperWork } from "@/lib/developers";
import { safeImage } from "@/lib/format";
import { appPath, authorPath } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./designs.module.css";
import grid from "./grid.module.css";

type Props = { app: App; work: DeveloperWork };

const lineOf = (app: App) => app.tagline?.trim() || firstSentence(app) || "No description yet.";
const moreTitle = (app: App) => `More by ${app.authors.length === 1 ? nameOf(app.authors[0]) : "these developers"}`;

function SeeAll({ app }: { app: App }) {
  return app.authors.length === 1 ? <a href={authorPath(app.authors[0].uuid)} className={styles.all}>See all</a> : null;
}

/** B · Shelf: who and where in two short rows, then More by as tiles that scroll sideways and snap. */
export function ShelfDesign({ app, work }: Props) {
  const links = linksOf(app);
  return (
    <section className={styles.section} aria-labelledby="developer-title">
      <h2 id="developer-title" className={styles.title}>{app.authors.length > 1 ? "Developers" : "Developer"}</h2>
      <ul className={styles.chips}>
        {app.authors.map(author => (
          <li key={author.uuid}>
            <a href={authorPath(author.uuid)} className={styles.personChip}>
              <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size="sm" aria-hidden="true" />
              <span className={styles.chipName}>{nameOf(author)}</span>
              <span className={styles.chipId}>{author.id}{work.totals[author.uuid] ? ` · ${appsCount(work.totals[author.uuid])}` : ""}</span>
            </a>
          </li>
        ))}
      </ul>
      {links.length ? (
        <ul className={styles.pills} aria-label="Links">
          {links.map((link, index) => {
            const Icon = link.icon;
            return <li key={index}><a href={link.url} className={styles.pill} title={link.host} target="_blank" rel="nofollow ugc noopener noreferrer"><Icon size={14} strokeWidth={2} aria-hidden="true" />{link.label}<ArrowUpRight className={styles.out} size={14} strokeWidth={2} aria-hidden="true" /></a></li>;
          })}
        </ul>
      ) : null}
      {work.more.length ? (
        <div className={styles.group}>
          <div className={styles.head}><h3 className={styles.subtitle}>{moreTitle(app)}</h3><SeeAll app={app} /></div>
          <ul className={styles.shelf} aria-label={moreTitle(app)}>
            {work.more.map(item => (
              <li key={item.app_id}>
                <a href={appPath(item.app_id)} className={styles.tile}>
                  <AppMark app={withAppIcon(item)} size={64} />
                  <span className={styles.tileName}>{item.name}</span>
                  <span className={styles.tileLine}>{lineOf(item)}</span>
                </a>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}

/** D · Byline: one sentence for who made it, the links as words after it, then More by as a row of icons and names. */
export function BylineDesign({ app, work }: Props) {
  const links = linksOf(app);
  // How many others, only for one developer: with several, their counts overlap and can't be added up.
  const total = app.authors.length === 1 ? work.totals[app.authors[0].uuid] ?? 0 : 0;
  return (
    <section className={styles.section} aria-labelledby="developer-title">
      <h2 id="developer-title" className={styles.title}>{app.authors.length > 1 ? "Developers" : "Developer"}</h2>
      <p className={styles.byline}>
        Made by{" "}
        {app.authors.map((author, index) => (
          <span key={author.uuid}>
            {index ? (index === app.authors.length - 1 ? " and " : ", ") : null}
            <a href={authorPath(author.uuid)} className={styles.byPerson} title={author.id}>
              <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size="sm" aria-hidden="true" />
              {nameOf(author)}
            </a>
          </span>
        ))}
        {total > 1 ? <>, with {total - 1} other {total - 1 === 1 ? "app" : "apps"} in the store.</> : "."}
      </p>
      {links.length ? (
        <p className={styles.words}>
          {links.map((link, index) => (
            <span key={index}>
              {index ? <span className={styles.dot} aria-hidden="true"> · </span> : null}
              <a href={link.url} title={link.host} target="_blank" rel="nofollow ugc noopener noreferrer">{link.label}</a>
            </span>
          ))}
        </p>
      ) : null}
      {work.more.length ? (
        <div className={styles.group}>
          <div className={styles.head}><h3 className={styles.subtitle}>{moreTitle(app)}</h3><SeeAll app={app} /></div>
          <ul className={styles.dock} aria-label={moreTitle(app)}>
            {work.more.map(item => (
              <li key={item.app_id}>
                <a href={appPath(item.app_id)} className={styles.dockItem} title={lineOf(item)}>
                  <AppMark app={withAppIcon(item)} size={56} />
                  <span>{item.name}</span>
                </a>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}

/** Grid, the page's first design (2026-11-18): the people as the store's app rows on its grid, the links as pills,
 * More by as the app grid under them. */
export function GridDesign({ app, work }: Props) {
  const links = linksOf(app);
  if (!app.authors.length && !links.length) return null;
  const single = app.authors.length === 1 ? app.authors[0] : null;
  return (
    <section className={grid.section} aria-labelledby="developer-title">
      <h2 id="developer-title" className={grid.title}>{app.authors.length > 1 ? "Developers" : "Developer"}</h2>

      {app.authors.length ? (
        <ul className={grid.people}>
          {app.authors.map(author => {
            const total = work.totals[author.uuid];
            return (
              <li key={author.uuid} className={grid.personRow}>
                <a href={authorPath(author.uuid)} className={grid.person}>
                  <Avatar name={nameOf(author)} src={safeImage(author.pfp_url)} size="lg" className={grid.face} aria-hidden="true" />
                  <span className={grid.who}>
                    <span className={grid.name}>{nameOf(author)}</span>
                    <span className={grid.id}>{author.id}{total ? <> · {appsCount(total)}</> : null}</span>
                  </span>
                </a>
              </li>
            );
          })}
        </ul>
      ) : null}

      {links.length ? (
        <ul className={grid.links} aria-label="Links">
          {links.map((link, index) => {
            const Icon = link.icon;
            return (
              <li key={`${link.url}-${index}`}>
                <a href={link.url} className={grid.link} title={link.host} target="_blank" rel="nofollow ugc noopener noreferrer">
                  <Icon size={14} strokeWidth={2} aria-hidden="true" />
                  {link.label}
                  <ArrowUpRight className={grid.out} size={14} strokeWidth={2} aria-hidden="true" />
                </a>
              </li>
            );
          })}
        </ul>
      ) : null}

      {work.more.length ? (
        <div className={grid.group}>
          <div className={grid.moreHead}>
            <h3 className={grid.subtitle}>More by {single ? nameOf(single) : "these developers"}</h3>
            {single ? <a href={authorPath(single.uuid)} className={grid.all}>See all</a> : null}
          </div>
          <Gallery apps={work.more} label={`More by ${single ? nameOf(single) : "these developers"}`} />
        </div>
      ) : null}
    </section>
  );
}
