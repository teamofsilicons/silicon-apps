/**
 * /apps/{app_id}: one app, read like an article on Silicon Writings. Its name and authors, the one command that
 * installs it, four facts, what it does (banner, description, tags, pictures), then plain label and value rows for its
 * links, releases (with withdrawn ones and their reasons), platforms, signatures and authors, and its reviews.
 * Server-rendered; writing, editing and removing a review are plain forms (server actions) that work without script.
 * Unknown apps, drafts and private apps the visitor may not see all answer 404 the same way (the API decides).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ArrowUpRight, ChevronLeft, ChevronRight, Lock, ShieldCheck, Star } from "lucide-react";
import buttonStyles from "@/components/silicon-ui/button/button.module.css";
import pageStyles from "@/components/store/app-page.module.css";
import { AppLogo, Avatar, Breadcrumbs, Command, Notice, Stars, StructuredData, TagList, styles } from "@/components/store/parts";
import { api, apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { formatDate, formatNumber, formatRating, plural, safeImage, safeUrl, targetLabel, targetsByOs, truncate } from "@/lib/format";
import { appPath, authorPath, breadcrumbLd, pageMetadata, softwareApplicationLd } from "@/lib/seo";
import { getAccount, signInHref } from "@/lib/session";
import { LINKS, installCommand, manageAppUrl } from "@/lib/site";
import type { App, Reviews } from "@/lib/types";
import { removeReview, saveReview } from "./actions";

type Props = { params: Promise<{ appId: string }>; searchParams: Promise<Record<string, string | string[] | undefined>> };

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
    image: { url: `${appPath(app.app_id)}/og.png`, alt: `${app.name} on Silicon Apps` },
    index: app.visibility === "public",
  });
}

const REVIEW_ERRORS: Record<string, string> = {
  rating: "Choose a rating from 1 to 5 stars.",
  too_long: "Keep your review to 600 characters or fewer.",
  sign_in: "Your session has ended. Sign in again, then save your review.",
  unavailable: "We could not reach the Apps service. Nothing was saved; try again in a moment.",
  rate_limited: "Too many requests right now. Wait a minute, then save your review again.",
};

type OutLink = { label: string; url: string; logo?: string };

function appLinks(app: App): OutLink[] {
  const raw = app.links ?? {};
  const list: Array<{ label: string; url?: string; logo?: string }> = [
    { label: "Website", url: raw.website },
    { label: "Developer docs", url: raw.developer_docs ?? raw.docs },
    { label: "Android app", url: raw.android },
    { label: "iOS app", url: raw.ios },
    ...(Array.isArray(raw.custom) ? raw.custom.slice(0, 6).map(link => ({ label: link.label || "Link", url: link.url, logo: link.logo })) : []),
  ];
  return list.flatMap(link => {
    const url = safeUrl(link.url);
    return url && /^https?:/.test(url) ? [{ label: link.label, url, logo: safeImage(link.logo) }] : [];
  });
}

export default async function AppPage({ params, searchParams }: Props) {
  const [{ appId }, query] = await Promise.all([params, searchParams]);
  const [app, account] = await Promise.all([load(appId), getAccount()]);
  if (!app || app.app_id !== appId) notFound();

  const reviews = await api<Reviews>(`/v1/apps/${encodeURIComponent(app.app_id)}/reviews`).catch((): Reviews => ({ items: [], rating: null, count: 0 }));
  const sorted = [...reviews.items].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  const mine = account ? reviews.items.find(review => review.uuid === account.uuid) : undefined;
  const others = sorted.filter(review => review !== mine);
  const path = appPath(app.app_id);
  const production = app.latest_production;
  const development = app.latest_development;
  const command = production || !development ? installCommand(app.app_id) : `silicon-apps install '${app.app_id}>dev'`;
  const banner = safeImage(app.banner);
  const media = (app.carousel ?? []).flatMap(item => {
    const url = safeImage(item.url);
    return url ? [{ ...item, url }] : [];
  });
  const outbound = appLinks(app);
  const status = typeof query.review === "string" ? query.review : "";
  const errorCode = typeof query.code === "string" ? query.code : "";
  const distribution = [5, 4, 3, 2, 1].map(stars => ({ stars, count: reviews.items.filter(review => review.rating === stars).length }));
  const platforms = targetsByOs(app.targets);
  const rated = reviews.count > 0 && reviews.rating !== null;
  const released = Boolean(production || development);
  const signedBy = app.signed_by ?? [];
  const withdrawn = app.withdrawn_releases ?? [];

  return (
    <div className={styles.page}>
      <StructuredData graph={[softwareApplicationLd(app, sorted), breadcrumbLd([{ name: "Home", path: "/" }, { name: "Apps", path: "/search" }, { name: app.name, path }])]} />
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "Apps", href: "/search" }, { name: app.name }]} />

      <article className={pageStyles.article} aria-labelledby="app-name">
        <header className={pageStyles.header}>
          <AppLogo app={app} size={64} decorative={false} />
          <div className={pageStyles.identity}>
            <h1 id="app-name" className={pageStyles.name}>{app.name}</h1>
            <p className={pageStyles.byline}>
              {app.authors.length ? (
                <span>
                  By{" "}
                  {app.authors.map((author, index) => (
                    <span key={author.uuid}>
                      {index > 0 ? ", " : null}
                      <a href={authorPath(author.uuid)} title={author.id} className={pageStyles.author}>
                        <Avatar name={author.display_name || author.id} id={author.id} photo={author.pfp_url} size={22} />
                        <span>{author.display_name || author.id}</span>
                      </a>
                    </span>
                  ))}
                </span>
              ) : null}
              <span className={pageStyles.appId}>{app.app_id}</span>
              {app.visibility === "private" ? (
                <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface"><Lock size={10} strokeWidth={2} aria-hidden="true" />Private app</span>
              ) : null}
            </p>
          </div>
        </header>

        <div className={pageStyles.install} id="install">
          <Command value={command} label="Copy the install command" large />
          <p className={pageStyles.installNote}>
            {production
              ? "Installs the latest production release for your system and keeps it up to date."
              : development
                ? "This app has development releases only so far, so this installs the latest one."
                : "There is no release to install yet."}{" "}
            New to it? <a href="/#get-the-cli">Get the CLI</a>.
          </p>
          {released && app.signed ? (
            <p className={pageStyles.trust}>
              <ShieldCheck size={15} strokeWidth={1.75} aria-hidden="true" />
              <span>
                Signed by Silicon Apps{app.signed_by_author && signedBy.length ? <> and by {signedBy.map((id, index) => <span key={id}>{index > 0 ? ", " : ""}<span className={pageStyles.mono}>{id}</span></span>)}</> : null}.
                The CLI checks every signature before it installs.
              </span>
            </p>
          ) : null}
        </div>

        <dl className={pageStyles.facts}>
          <div>
            <dt>Rating</dt>
            <dd>{rated ? <>{formatRating(reviews.rating)} <Stars rating={reviews.rating!} size={12} /></> : <span className={pageStyles.quiet}>None yet</span>}</dd>
          </div>
          <div>
            <dt>Installs</dt>
            <dd>{formatNumber(app.installs)}</dd>
          </div>
          <div>
            <dt>Version</dt>
            <dd>{production ? production.version : development ? development.version : <span className={pageStyles.quiet}>None</span>}</dd>
          </div>
          <div>
            <dt>Platforms</dt>
            <dd>{platforms.map(group => group.os).join(", ") || <span className={pageStyles.quiet}>None yet</span>}</dd>
          </div>
        </dl>

        {banner ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img className={pageStyles.banner} data-sq="clip" src={banner} alt={app.banner_alt || ""} />
        ) : null}

        <section aria-labelledby="about-title" className={pageStyles.section}>
          <h2 id="about-title" className={pageStyles.sectionTitle}>About {app.name}</h2>
          <div className={pageStyles.description}>
            {(app.description || "The authors have not written a description yet.").split(/\n{2,}/).map((paragraph, index) => (
              <p key={index}>{paragraph}</p>
            ))}
          </div>
          <TagList tags={app.tags} label={`${app.name} tags`} />
        </section>

        {media.length ? (
          <section aria-labelledby="media-title" className={pageStyles.section} data-carousel="">
            <div className={pageStyles.sectionBar}>
              <h2 id="media-title" className={pageStyles.sectionTitle}>A closer look</h2>
              <div className={pageStyles.carouselControls} data-js-only="">
                <button type="button" className={pageStyles.carouselButton} data-sq="surface" data-carousel-step="-1" aria-controls="app-media" aria-label="Previous picture">
                  <ChevronLeft size={16} strokeWidth={1.75} aria-hidden="true" />
                </button>
                <button type="button" className={pageStyles.carouselButton} data-sq="surface" data-carousel-step="1" aria-controls="app-media" aria-label="Next picture">
                  <ChevronRight size={16} strokeWidth={1.75} aria-hidden="true" />
                </button>
              </div>
            </div>
            <ul className={pageStyles.track} id="app-media" role="list" tabIndex={0} aria-label={`${app.name} pictures and videos`}>
              {media.map((item, index) => (
                <li key={`${item.url}-${index}`} className={pageStyles.slide} data-sq="clip">
                  {item.kind === "video" ? (
                    <video controls preload="metadata" src={item.url} aria-label={item.alt || `${app.name} video ${index + 1}`} />
                  ) : (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img src={item.url} alt={item.alt || `${app.name} picture ${index + 1}`} loading="lazy" decoding="async" />
                  )}
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <div className={pageStyles.details}>
          {outbound.length ? (
            <section aria-labelledby="links-title" className={pageStyles.detail}>
              <h2 id="links-title" className={pageStyles.label}>Links</h2>
              <ul className={pageStyles.links} role="list">
                {outbound.map((link, index) => (
                  <li key={`${link.url}-${index}`}>
                    <a href={link.url} target="_blank" rel="nofollow ugc noopener noreferrer">
                      {link.label}<ArrowUpRight size={13} strokeWidth={1.75} aria-hidden="true" />
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          <section aria-labelledby="channels-title" className={pageStyles.detail}>
            <h2 id="channels-title" className={pageStyles.label}>Releases</h2>
            <div className={pageStyles.value}>
              {production || development ? (
                <dl className={pageStyles.releases}>
                  {production ? (
                    <div>
                      <dt><span className={pageStyles.mono}>{production.version}</span> Production, {formatDate(production.created_at)}</dt>
                      {production.notes ? <dd>{truncate(production.notes, 220)}</dd> : null}
                    </div>
                  ) : null}
                  {development ? (
                    <div>
                      <dt><span className={pageStyles.mono}>{development.version}</span> Development, {formatDate(development.created_at)}</dt>
                      {development.notes ? <dd>{truncate(development.notes, 220)}</dd> : null}
                    </div>
                  ) : null}
                </dl>
              ) : (
                <p>No release yet. Check back soon.</p>
              )}
              {withdrawn.length ? (
                <div className={pageStyles.withdrawn}>
                  <h3 className={pageStyles.subLabel}>Withdrawn</h3>
                  <ul role="list">
                    {withdrawn.map(item => (
                      <li key={item.release_id}>
                        <p><span className={`${pageStyles.mono} ${pageStyles.struck}`}>{item.version}</span> {item.channel === "production" ? "Production" : "Development"}, <time dateTime={item.withdrawn_at}>{formatDate(item.withdrawn_at)}</time></p>
                        {item.reason ? <p className={pageStyles.muted}>{item.reason}</p> : null}
                      </li>
                    ))}
                  </ul>
                  <p className={pageStyles.muted}>A withdrawn release is never installed again, and updates move every copy of it to the latest good release.</p>
                </div>
              ) : null}
              {development || production ? (
                <details className={pageStyles.more}>
                  <summary>Other ways to install</summary>
                  <div className={pageStyles.moreBody}>
                    {development ? (
                      <div>
                        <p>The latest development release</p>
                        <Command value={`silicon-apps install '${app.app_id}>dev'`} label="Copy the development install command" wrap />
                      </div>
                    ) : null}
                    {production ? (
                      <div>
                        <p>This exact version</p>
                        <Command value={`silicon-apps install '${app.app_id}@${production.version}'`} label="Copy the exact version install command" wrap />
                      </div>
                    ) : null}
                    <p className={pageStyles.muted}>The CLI asks before switching between production and development. An exact version picks what you install first; updates still follow its channel.</p>
                  </div>
                </details>
              ) : null}
            </div>
          </section>

          {platforms.length ? (
            <section aria-labelledby="platforms-title" className={pageStyles.detail}>
              <h2 id="platforms-title" className={pageStyles.label}>Available on</h2>
              <ul className={pageStyles.platforms} role="list">
                {platforms.map(group => (
                  <li key={group.os}>
                    <span>{group.os}</span>{" "}
                    <span className={pageStyles.muted}>{group.targets.map(target => targetLabel(target).replace(`${group.os} `, "")).join(", ")}</span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {released ? (
            <section aria-labelledby="signatures-title" className={pageStyles.detail}>
              <h2 id="signatures-title" className={pageStyles.label}>Signatures</h2>
              <div className={pageStyles.value}>
                <p>Silicon Apps: <span data-state={app.signed ? "yes" : "no"} className={pageStyles.state}>{app.signed ? "Signed every package" : "Not signed yet"}</span></p>
                <p>Authors: <span data-state={signedBy.length ? "yes" : "no"} className={pageStyles.state}>{signedBy.length ? `${app.signed_by_author ? "All" : "Some"} packages signed by ${signedBy.join(", ")}` : "No additional author signatures"}</span></p>
                <p className={pageStyles.muted}>
                  For the current {production ? "production" : "development"} release. <a href={`${LINKS.appsDocs}/learn/signed-releases`} target="_blank" rel="noopener noreferrer">How signed releases work</a>
                </p>
              </div>
            </section>
          ) : null}

          {app.authors.length ? (
            <section aria-labelledby="authors-title" className={pageStyles.detail}>
              <h2 id="authors-title" className={pageStyles.label}>Authors</h2>
              <ul className={pageStyles.people} role="list">
                {app.authors.map(author => (
                  <li key={author.uuid}>
                    <a href={authorPath(author.uuid)} className={pageStyles.person}>
                      <Avatar name={author.display_name || author.id} id={author.id} photo={author.pfp_url} size={28} />
                      <span>{author.display_name || author.id}</span>
                      <span className={pageStyles.personId}>{author.id}</span>
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </div>

        <section aria-labelledby="reviews-title" className={pageStyles.section} id="reviews">
          <div className={pageStyles.sectionBar}>
            <h2 id="reviews-title" className={pageStyles.sectionTitle}>Ratings and reviews</h2>
            {app.is_author ? <a className={pageStyles.manage} href={manageAppUrl(app.app_id)} target="_blank" rel="noopener noreferrer">Manage app<ArrowUpRight size={13} strokeWidth={1.75} aria-hidden="true" /></a> : null}
          </div>
          {rated ? (
          <div className={pageStyles.summary}>
            <div className={pageStyles.score}>
              <span className={pageStyles.average}>{formatRating(reviews.rating)}</span>
              <Stars rating={reviews.rating!} size={14} />
              <span className={pageStyles.muted}>{plural(reviews.count, "review")}</span>
            </div>
            <ul className={pageStyles.bars} role="list" aria-label="Reviews by stars">
              {distribution.map(({ stars, count }) => (
                <li key={stars}>
                  <span className={pageStyles.barLabel} aria-hidden="true">{stars}</span>
                  <span className={pageStyles.bar} aria-hidden="true">
                    <span style={{ width: `${reviews.count ? (count / reviews.count) * 100 : 0}%` }} />
                  </span>
                  <span className="sr-only">{stars} {stars === 1 ? "star" : "stars"}: {plural(count, "review")}</span>
                </li>
              ))}
            </ul>
          </div>
          ) : null}

          {status === "saved" ? <Notice tone="success">Your review is saved. Thank you for sharing it.</Notice> : null}
          {status === "removed" ? <Notice tone="success">Your review is removed. You can write a new one any time.</Notice> : null}
          {status === "error" ? (
            <Notice tone="danger" title="Nothing was saved">
              <p>{REVIEW_ERRORS[errorCode] ?? "We could not save your review. Check it and try again."}</p>
            </Notice>
          ) : null}

          {account ? (
            <form action={saveReview} className={pageStyles.reviewForm} data-sq="surface" aria-labelledby="review-form-title">
              <h3 id="review-form-title" className={pageStyles.formTitle}>{mine ? "Edit your review" : `Review ${app.name}`}</h3>
              <p className={pageStyles.muted}>
                Reviewing as <span className={pageStyles.mono}>{account.id}</span>. One review per app; change or remove it any time.
              </p>
              <input type="hidden" name="app_id" value={app.app_id} />
              <input type="hidden" name="idempotency_key" value={crypto.randomUUID()} />
              <fieldset className={pageStyles.rating}>
                <legend className={styles.label}>Your rating</legend>
                <div className={pageStyles.ratingStars}>
                  {[1, 2, 3, 4, 5].map(n => (
                    <label key={n} className={pageStyles.ratingStar}>
                      <input type="radio" name="rating" value={n} defaultChecked={(mine?.rating ?? 0) === n} required />
                      <Star size={26} strokeWidth={1.5} aria-hidden="true" />
                      <span className="sr-only">{n === 1 ? "1 star" : `${n} stars`}</span>
                    </label>
                  ))}
                </div>
              </fieldset>
              <div className={styles.field}>
                <label htmlFor="review-text" className={styles.label}>Your review <span className={styles.muted}>(optional)</span></label>
                <textarea id="review-text" name="text" className={styles.textarea} data-sq="surface" maxLength={600} rows={4} defaultValue={mine?.text ?? ""} aria-describedby="review-text-hint" placeholder="What does it do well? What could be better?" />
                <p id="review-text-hint" className={styles.hint} data-count-for="review-text">Up to 600 characters.</p>
              </div>
              <div className={pageStyles.formActions}>
                <button type="submit" className={`${buttonStyles.button} ${buttonStyles.primary} ${buttonStyles.md}`} data-sq="surface">{mine ? "Save changes" : "Post review"}</button>
              </div>
            </form>
          ) : (
            <p className={pageStyles.signIn}>
              Used {app.name}? <a href={signInHref(`${path}#reviews`)} rel="nofollow">Sign in to write a review</a>
            </p>
          )}

          {mine ? (
            <details className={pageStyles.remove}>
              <summary>Remove your review</summary>
              <form action={removeReview} className={pageStyles.removeForm}>
                <input type="hidden" name="app_id" value={app.app_id} />
                <input type="hidden" name="idempotency_key" value={crypto.randomUUID()} />
                <p>This deletes your rating and text for {app.name}. You can write a new review later.</p>
                <button type="submit" className={`${buttonStyles.button} ${buttonStyles.danger} ${buttonStyles.sm}`} data-sq="surface">Remove my review</button>
              </form>
            </details>
          ) : null}

          {mine || others.length ? (
            <ul className={pageStyles.reviews} role="list" aria-label="Reviews">
              {[...(mine ? [mine] : []), ...others].map(review => (
                <li key={review.uuid}>
                  <article className={pageStyles.review} data-mine={review === mine ? "" : undefined} aria-label={review === mine ? "Your review" : `Review by ${review.id}`}>
                    <header className={pageStyles.reviewHead}>
                      <Avatar name={review.id} id={review.id} size={24} />
                      <span className={pageStyles.mono}>{review.id}</span>
                      {review === mine ? <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface">You</span> : null}
                      <Stars rating={review.rating} size={12} />
                      <time dateTime={review.updated_at} className={pageStyles.reviewDate}>{formatDate(review.updated_at)}</time>
                    </header>
                    {review.text ? <p className={pageStyles.reviewText}>{review.text}</p> : null}
                  </article>
                </li>
              ))}
            </ul>
          ) : (
            <p className={pageStyles.muted}>No reviews yet. Be the first to say what you think.</p>
          )}
        </section>
      </article>
    </div>
  );
}
