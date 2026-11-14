/**
 * /apps/{app_id}: one app (Anil's sketch, 2026-11-14; the live store's content and flow, in the account site's style):
 * - a cover across the column (the app's own picture, else its banner, else its first picture, else a quiet one), the
 *   app's icon overlapping its lower-left corner, and beside the icon the app's name and "By" its authors, each a link
 *   to their page, then its id and its category (a link to the category's page);
 * - "Try it": its website as the main button where it has one, then the one command that installs it, with a copy
 *   button (and Manage App for its authors);
 * - the facts in a row: rating, installs, version, platforms (the systems' marks);
 * - a hairline, then About (the description and tags);
 * - the details as labelled rows, as the live store has them: Links, Releases (production and development with their
 *   notes, who signed them, withdrawn releases, other ways to install), Available on (each system with its
 *   architectures), Authors;
 * - Ratings and reviews: the average, the spread by stars, every review; signing in to write one (the form comes back
 *   next). Server-rendered; apps the visitor may not see answer 404 (proxy.ts asks the API
 * first, and this page asks again).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ArrowUpRight } from "lucide-react";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { Command } from "@/components/app/command";
import { CategoryIcon } from "@/components/parts/category-icon";
import styles from "@/components/app/app-page.module.css";
import { AppMark } from "@/components/home/app-mark";
import { OsIcons } from "@/components/home/os-icons";
import { Page } from "@/components/parts/page";
import { api, apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { withAppIcon } from "@/lib/app-icons";
import { APP_PICTURES } from "@/lib/app-pictures";
import { categoryOf } from "@/lib/categories";
import { formatCount, formatDate, formatRating, operatingSystems, plural, safeImage, safeUrl, targetLabel, targetsByOs, truncate } from "@/lib/format";
import { appPath, authorPath, pageMetadata } from "@/lib/seo";
import { getAccount, signInHref } from "@/lib/session";
import { installCommand, LINKS, manageAppUrl } from "@/lib/site";
import type { App, Reviews } from "@/lib/types";

type Props = { params: Promise<{ appId: string }> };

async function load(appId: string): Promise<App | null> {
  if (!APP_ID_PATTERN.test(appId)) return null;
  return apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`);
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { appId } = await params;
  const app = await load(appId).catch(() => null);
  if (!app) return { title: { absolute: "App not found · Silicon Apps" }, robots: { index: false, follow: true } };
  return pageMetadata({
    title: app.name,
    description: app.description || `${app.name} on Silicon Apps. Install it with ${installCommand(app.app_id)}.`,
    path: appPath(app.app_id),
    index: app.visibility === "public",
  });
}

/** The app's links, website first, then its own, as label and address (only http and https). */
function linksOf(app: App): { label: string; url: string }[] {
  const raw = app.links ?? {};
  const list = [
    { label: "Website", url: raw.website },
    { label: "Developer docs", url: raw.developer_docs ?? raw.docs },
    { label: "Android app", url: raw.android },
    { label: "iOS app", url: raw.ios },
    ...(Array.isArray(raw.custom) ? raw.custom.slice(0, 6).map(link => ({ label: link.label || "Link", url: link.url })) : []),
  ];
  return list.flatMap(link => {
    const url = safeUrl(link.url);
    return url && /^https?:/.test(url) ? [{ label: link.label, url }] : [];
  });
}

/** The cover: the app's own picture, else its banner, else its first picture, else one of the account site's. */
function coverOf(app: App): { src: string; focus?: string } {
  const own = APP_PICTURES[app.app_id];
  if (own) return own;
  const banner = safeImage(app.banner);
  if (banner) return { src: banner };
  const first = (app.carousel ?? []).find(item => item.kind !== "video" && safeImage(item.url));
  if (first) return { src: safeImage(first.url)! };
  return { src: "/banner/bg-110.webp" };
}

export default async function AppPage({ params }: Props) {
  const { appId } = await params;
  const [app, account] = await Promise.all([load(appId), getAccount()]);
  if (!app || app.app_id !== appId) notFound();
  const reviews = await api<Reviews>(`/v1/apps/${encodeURIComponent(app.app_id)}/reviews`).catch((): Reviews => ({ items: [], rating: null, count: 0 }));
  const sortedReviews = [...reviews.items].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  const rated = reviews.count > 0 && reviews.rating !== null;
  const distribution = [5, 4, 3, 2, 1].map(stars => ({ stars, count: reviews.items.filter(review => review.rating === stars).length }));
  const withdrawn = app.withdrawn_releases ?? [];
  const signedBy = app.signed_by ?? [];
  const platforms = targetsByOs(app.targets);

  const release = app.latest_production ?? app.latest_development;
  const command = app.latest_production || !app.latest_development ? installCommand(app.app_id) : `silicon-apps install '${app.app_id}>dev'`;
  const systems = operatingSystems(app.targets);
  const links = linksOf(app);
  const paragraphs = app.description ? app.description.split(/\n{2,}/) : [];

  const cover = coverOf(app);
  const category = categoryOf(app.app_id);
  const website = links.find(link => link.label === "Website");
  const name = (author: App["authors"][number]) => author.display_name || author.id;

  return (
    <article aria-labelledby="app-name">
      <Page>
        <header className={styles.head}>
          {/* eslint-disable-next-line @next/next/no-img-element -- the app's picture, shown as it is. */}
          <img className={styles.cover} data-sq="clip" src={cover.src} alt="" style={{ objectPosition: cover.focus ?? "center" }} decoding="async" fetchPriority="high" />
          <div className={styles.identity}>
            <span className={styles.icon}><AppMark app={withAppIcon(app)} size={96} /></span>
            <div className={styles.names}>
              <h1 id="app-name" className={styles.name}>
                {app.name}
                {app.visibility === "private" ? <Badge size="sm" tone="neutral">Private</Badge> : null}
              </h1>
              {app.authors.length ? (
                <p className={styles.by}>
                  <span>By</span>
                  {app.authors.map((author, index) => (
                    <span key={author.uuid} className={styles.byAuthor}>
                      <a href={authorPath(author.uuid)} title={author.id}>
                        <Avatar name={name(author)} src={safeImage(author.pfp_url)} size="sm" aria-hidden="true" />
                        {name(author)}
                      </a>
                      {index < app.authors.length - 1 ? "," : null}
                    </span>
                  ))}
                  <span className={styles.appId}>{app.app_id}</span>
                  {category ? (
                    <a href={`/categories/${category.id}`} className={styles.category}>
                      <CategoryIcon id={category.id} size={14} />{category.title}
                    </a>
                  ) : null}
                </p>
              ) : null}
            </div>
          </div>
        </header>

        <section className={styles.try} aria-labelledby="try-title">
          <h2 id="try-title" className={styles.tryLabel}>Try it</h2>
          <div className={styles.tryRow}>
            {website ? (
              <ButtonLink href={website.url} target="_blank" rel="nofollow ugc noopener noreferrer">
                Open Website<ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
              </ButtonLink>
            ) : null}
            <div className={styles.tryCommand}><Command value={command} label={`Copy the command that installs ${app.name}`} /></div>
            {app.is_author ? (
              <ButtonLink href={manageAppUrl(app.app_id)} variant="secondary" target="_blank" rel="noopener noreferrer">
                Manage App<ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
              </ButtonLink>
            ) : null}
          </div>
          <p className={styles.tryNote}>
            {!release ? "There is no release to install yet." : !app.latest_production ? "Only development releases so far, so this installs the latest one." : "Installs the latest production release for your system and keeps it up to date."}{" "}
            New to it? <a href={LINKS.installDocs} target="_blank" rel="noopener noreferrer">Get the CLI</a>.
          </p>
        </section>

        <dl className={styles.facts}>
          <div>
            <dt>Rating</dt>
            <dd>{rated ? <>{formatRating(reviews.rating)} <span className={styles.star} aria-hidden="true">★</span></> : <span className={styles.quiet}>None yet</span>}</dd>
          </div>
          <div>
            <dt>Installs</dt>
            <dd>{formatCount(app.installs)}</dd>
          </div>
          <div>
            <dt>Version</dt>
            <dd>{release ? <span className={styles.mono}>{release.version}</span> : <span className={styles.quiet}>None yet</span>}</dd>
          </div>
          <div>
            <dt>Platforms</dt>
            <dd>{systems.length ? <span className={styles.platformsFact}><OsIcons targets={app.targets} /><span className={styles.platformsNames}>{systems.join(", ")}</span></span> : <span className={styles.quiet}>None yet</span>}</dd>
          </div>
        </dl>

        <section className={styles.about} aria-labelledby="about-title">
          <h2 id="about-title" className={styles.sectionTitle}>About</h2>
          <div className={styles.prose}>
            {paragraphs.length ? paragraphs.map((paragraph, index) => <p key={index}>{paragraph}</p>) : <p className={styles.quiet}>The authors have not written a description yet.</p>}
          </div>
          {app.tags.length ? (
            <ul className={styles.tags} aria-label="Tags">
              {app.tags.map(tag => <li key={tag}><Badge size="sm" tone="neutral">{tag}</Badge></li>)}
            </ul>
          ) : null}
        </section>

        <div className={styles.details}>
          {links.length ? (
            <section className={styles.detail} aria-labelledby="links-title">
              <h2 id="links-title" className={styles.label}>Links</h2>
              <ul className={styles.links}>
                {links.map((link, index) => (
                  <li key={`${link.url}-${index}`}>
                    <a href={link.url} target="_blank" rel="nofollow ugc noopener noreferrer">{link.label}<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /></a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          <section className={styles.detail} aria-labelledby="releases-title">
            <h2 id="releases-title" className={styles.label}>Releases</h2>
            <div className={styles.value}>
              {app.latest_production || app.latest_development ? (
                <dl className={styles.releases}>
                  {[app.latest_production, app.latest_development].flatMap(item => (item ? [item] : [])).map(item => (
                    <div key={item.id}>
                      <dt><span className={styles.mono}>{item.version}</span> {item.channel === "production" ? "Production" : "Development"}, {formatDate(item.created_at)}</dt>
                      {item.notes ? <dd>{truncate(item.notes, 220)}</dd> : null}
                    </div>
                  ))}
                </dl>
              ) : <p className={styles.muted}>No release yet. Check back soon.</p>}
              {release ? (
                <p className={styles.muted}>
                  {app.signed ? <>Signed by Silicon Apps{signedBy.length ? <> and by {signedBy.join(", ")}</> : null}.</> : "Not signed yet."}{" "}
                  <a href={`${LINKS.appsDocs}/learn/signed-releases`} target="_blank" rel="noopener noreferrer">How signing works</a>
                </p>
              ) : null}
              {withdrawn.length ? (
                <div className={styles.withdrawn}>
                  <h3 className={styles.subLabel}>Withdrawn</h3>
                  <ul>
                    {withdrawn.map(item => (
                      <li key={item.release_id}>
                        <p><span className={`${styles.mono} ${styles.struck}`}>{item.version}</span> {item.channel === "production" ? "Production" : "Development"}, {formatDate(item.withdrawn_at)}</p>
                        {item.reason ? <p className={styles.muted}>{item.reason}</p> : null}
                      </li>
                    ))}
                  </ul>
                  <p className={styles.muted}>A withdrawn release is never installed again, and updates move every copy of it to the latest good release.</p>
                </div>
              ) : null}
              {release ? (
                <details className={styles.more}>
                  <summary>Other ways to install</summary>
                  <div className={styles.moreBody}>
                    {app.latest_development ? (
                      <div>
                        <p>The latest development release</p>
                        <Command value={`silicon-apps install '${app.app_id}>dev'`} label="Copy the development install command" />
                      </div>
                    ) : null}
                    {app.latest_production ? (
                      <div>
                        <p>This exact version</p>
                        <Command value={`silicon-apps install '${app.app_id}@${app.latest_production.version}'`} label="Copy the exact version install command" />
                      </div>
                    ) : null}
                    <p className={styles.muted}>The CLI asks before switching between production and development. An exact version picks what you install first; updates still follow its channel.</p>
                  </div>
                </details>
              ) : null}
            </div>
          </section>

          {platforms.length ? (
            <section className={styles.detail} aria-labelledby="platforms-title">
              <h2 id="platforms-title" className={styles.label}>Available on</h2>
              <ul className={styles.platforms}>
                {platforms.map(group => (
                  <li key={group.os}>
                    <OsIcons targets={group.targets} />
                    <span>{group.os}</span>{" "}
                    <span className={styles.muted}>{group.targets.map(target => targetLabel(target).replace(`${group.os} `, "")).join(", ")}</span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {app.authors.length ? (
            <section className={styles.detail} aria-labelledby="authors-title">
              <h2 id="authors-title" className={styles.label}>Authors</h2>
              <ul className={styles.people}>
                {app.authors.map(author => (
                  <li key={author.uuid}>
                    <a href={authorPath(author.uuid)} className={styles.person}>
                      <Avatar name={name(author)} src={safeImage(author.pfp_url)} size="sm" aria-hidden="true" />
                      <span>{name(author)}</span>
                      <span className={styles.personId}>{author.id}</span>
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </div>

        <section className={styles.reviews} id="reviews" aria-labelledby="reviews-title">
          <h2 id="reviews-title" className={styles.sectionTitle}>Ratings and reviews</h2>
          {rated ? (
            <div className={styles.summary}>
              <p className={styles.average}><span>{formatRating(reviews.rating)}</span> <span className={styles.star} aria-hidden="true">★</span></p>
              <p className={styles.muted}>{plural(reviews.count, "review")}</p>
              <ul className={styles.bars} aria-label="Ratings by stars">
                {distribution.map(row => (
                  <li key={row.stars}>
                    <span className={styles.barLabel}>{row.stars} ★</span>
                    <span className={styles.bar}><span style={{ width: `${reviews.count ? (row.count / reviews.count) * 100 : 0}%` }} /></span>
                    <span className={styles.barCount}>{row.count}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {account ? (
            <p className={styles.muted}>Writing a review here comes back soon. For now, use <code className={styles.mono}>silicon-apps review {app.app_id}</code>.</p>
          ) : (
            <p className={styles.muted}>Used {app.name}? <a href={signInHref(`/apps/${encodeURIComponent(app.app_id)}#reviews`)} rel="nofollow">Sign in to write a review</a></p>
          )}
          {sortedReviews.length ? (
            <ul className={styles.reviewList} aria-label="Reviews">
              {sortedReviews.map(review => (
                <li key={review.uuid} className={styles.review}>
                  <div className={styles.reviewHead}>
                    <Avatar name={review.id.replace(/^(c|si):/, "")} size="sm" aria-hidden="true" />
                    <span className={styles.mono}>{review.id}</span>
                    <span>{review.rating} <span className={styles.star} aria-hidden="true">★</span></span>
                    <span className={styles.reviewDate}>{formatDate(review.updated_at)}</span>
                  </div>
                  {review.text ? <p className={styles.reviewText}>{review.text}</p> : null}
                </li>
              ))}
            </ul>
          ) : (
            <p className={styles.muted}>No reviews yet. Be the first to say what you think.</p>
          )}
        </section>
      </Page>
    </article>
  );
}
