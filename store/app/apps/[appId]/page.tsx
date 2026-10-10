/**
 * /apps/{app_id}: everything an app's authors set up, server-rendered: name, logo, banner, carousel (with alt text),
 * description, tags, links, authors, platforms, latest releases, who signed them, withdrawn releases with their
 * reasons, rating, installs, the one command that installs it, and reviews. Writing, editing and removing a review are plain forms (server actions) that work without script.
 * Unknown apps, drafts and private apps the visitor may not see all answer 404 the same way (the API decides).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ArrowUpRight, BookOpen, ChevronLeft, ChevronRight, ExternalLink, Globe, Layers, Lock, Package, ShieldCheck, Smartphone, Star, Users } from "lucide-react";
import buttonStyles from "@/components/silicon-ui/button/button.module.css";
import pageStyles from "@/components/store/app-page.module.css";
import { AppLogo, Avatar, Breadcrumbs, Command, Notice, Stars, StructuredData, TagList, accountKind, styles } from "@/components/store/parts";
import { Action } from "@/components/site/action";
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

type OutLink = { label: string; url: string; logo?: string; icon: "web" | "docs" | "phone" | "link" };

function appLinks(app: App): OutLink[] {
  const raw = app.links ?? {};
  const list: Array<{ label: string; url?: string; logo?: string; icon: OutLink["icon"] }> = [
    { label: "Website", url: raw.website, icon: "web" },
    { label: "Developer docs", url: raw.developer_docs ?? raw.docs, icon: "docs" },
    { label: "Android app", url: raw.android, icon: "phone" },
    { label: "iOS app", url: raw.ios, icon: "phone" },
    ...(Array.isArray(raw.custom) ? raw.custom.slice(0, 6).map(link => ({ label: link.label || "Link", url: link.url, logo: link.logo, icon: "link" as const })) : []),
  ];
  return list.flatMap(link => {
    const url = safeUrl(link.url);
    return url && /^https?:/.test(url) ? [{ label: link.label, url, logo: safeImage(link.logo), icon: link.icon }] : [];
  });
}

const LINK_ICONS = { web: Globe, docs: BookOpen, phone: Smartphone, link: ExternalLink } as const;
const ICON = { size: 17, strokeWidth: 1.75 } as const;

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
  const signedBy = app.signed_by_author ? (app.signed_by ?? []) : [];
  const withdrawn = app.withdrawn_releases ?? [];

  return (
    <div className={`${styles.page} ${pageStyles.page}`}>
      <StructuredData graph={[softwareApplicationLd(app, sorted), breadcrumbLd([{ name: "Home", path: "/" }, { name: "All apps", path: "/search" }, { name: app.name, path }])]} />
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "All apps", href: "/search" }, { name: app.name }]} />

      <article className={pageStyles.article} aria-labelledby="app-name">
        {banner ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img className={pageStyles.banner} data-sq="clip" src={banner} alt={app.banner_alt || ""} />
        ) : null}

        <header className={pageStyles.header}>
          <div className={pageStyles.identityRow}>
            <AppLogo app={app} size={88} decorative={false} />
            <div className={pageStyles.identity}>
              <h1 id="app-name" className={pageStyles.name}>{app.name}</h1>
              {app.authors.length ? (
                <p className={pageStyles.authors}>
                  <span className="sr-only">By </span>
                  {app.authors.map((author, index) => (
                    <span key={author.uuid}>
                      {index > 0 ? <span aria-hidden="true">, </span> : null}
                      <a href={authorPath(author.uuid)} title={author.id}>{author.display_name || author.id}</a>
                    </span>
                  ))}
                </p>
              ) : null}
              <p className={pageStyles.idLine}>
                <span className={pageStyles.appId}>{app.app_id}</span>
                {app.visibility === "private" ? (
                  <span className={`${styles.badge} ${styles.badgeAccent}`} data-sq="surface"><Lock size={11} strokeWidth={2} aria-hidden="true" />Private app</span>
                ) : (
                  <span className={styles.badge} data-sq="surface">Public app</span>
                )}
              </p>
            </div>
          </div>
          <div className={pageStyles.cta} id="install">
            <p className={pageStyles.ctaLabel}>Install with one command</p>
            <Command value={command} label="Copy the install command" large />
            <p className={pageStyles.ctaNote}>
              {production
                ? "Installs the latest production release for your system and keeps it up to date."
                : development
                  ? "This app has development releases only so far, so this installs the latest one."
                  : "There is no release to install yet."}{" "}
              New to it? <a href="/#get-the-cli">Get the CLI</a>.
            </p>
            {released && app.signed ? (
              <p className={pageStyles.trust}>
                <ShieldCheck size={16} strokeWidth={1.75} aria-hidden="true" />
                <span>
                  Signed by Silicon Apps{signedBy.length ? <> and by {signedBy.map((id, index) => <span key={id}>{index > 0 ? ", " : ""}<span className={pageStyles.signer}>{id}</span></span>)}</> : null}.
                  The CLI checks {signedBy.length ? "both signatures" : "the signature"} before it installs anything.
                </span>
              </p>
            ) : null}
            <div className={pageStyles.ctaActions}>
              <Action href="#reviews" variant="secondary" size="sm"><Star size={15} strokeWidth={1.75} aria-hidden="true" />{mine ? "Edit your review" : "Write a review"}</Action>
              {app.is_author ? <Action href={manageAppUrl(app.app_id)} variant="ghost" size="sm">Manage app<ArrowUpRight size={15} strokeWidth={1.75} aria-hidden="true" /></Action> : null}
            </div>
          </div>
        </header>

        <dl className={pageStyles.stats} data-sq="surface">
          <div>
            <dt>Rating</dt>
            <dd>
              {rated ? (
                <>
                  <span className={pageStyles.statValue}>{formatRating(reviews.rating)}</span>
                  <Stars rating={reviews.rating!} size={13} />
                  <span className={pageStyles.statNote}>{plural(reviews.count, "review")}</span>
                </>
              ) : (
                <>
                  <span className={`${pageStyles.statValue} ${pageStyles.statQuiet}`}>None yet</span>
                  <span className={pageStyles.statNote}>Be the first to review it</span>
                </>
              )}
            </dd>
          </div>
          <div>
            <dt>Installs</dt>
            <dd>
              <span className={pageStyles.statValue}>{formatNumber(app.installs)}</span>
              <span className={pageStyles.statNote}>Every finished install</span>
            </dd>
          </div>
          <div>
            <dt>Latest release</dt>
            <dd>
              <span className={pageStyles.statValue}>{production ? production.version : development ? development.version : "None"}</span>
              <span className={pageStyles.statNote}>{production ? `Production, ${formatDate(production.created_at)}` : development ? "Development only" : "No release yet"}</span>
            </dd>
          </div>
          <div>
            <dt>Platforms</dt>
            <dd>
              <span className={pageStyles.statValue}>{app.targets.length}</span>
              <span className={pageStyles.statNote}>{platforms.map(group => group.os).join(", ") || "None yet"}</span>
            </dd>
          </div>
        </dl>

        <div className={pageStyles.layout}>
          <div className={pageStyles.main}>
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
                      <ChevronLeft size={18} strokeWidth={1.75} aria-hidden="true" />
                    </button>
                    <button type="button" className={pageStyles.carouselButton} data-sq="surface" data-carousel-step="1" aria-controls="app-media" aria-label="Next picture">
                      <ChevronRight size={18} strokeWidth={1.75} aria-hidden="true" />
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

            <section aria-labelledby="reviews-title" className={pageStyles.section} id="reviews">
              <h2 id="reviews-title" className={pageStyles.sectionTitle}>Ratings and reviews</h2>
              <div className={pageStyles.summary} data-sq="surface">
                <div className={pageStyles.score}>
                  <span className={rated ? pageStyles.average : `${pageStyles.average} ${pageStyles.averageQuiet}`}>{rated ? formatRating(reviews.rating) : "None yet"}</span>
                  {rated ? <Stars rating={reviews.rating!} size={16} /> : null}
                  <span className={pageStyles.statNote}>{reviews.count ? plural(reviews.count, "review") : "No reviews yet"}</span>
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
                  <p className={pageStyles.formNote}>
                    Reviewing as <strong>{account.id}</strong>. You have one review per app, and you can change or remove it any time.
                  </p>
                  <input type="hidden" name="app_id" value={app.app_id} />
                  <input type="hidden" name="idempotency_key" value={crypto.randomUUID()} />
                  <fieldset className={pageStyles.rating}>
                    <legend className={styles.label}>Your rating</legend>
                    <div className={pageStyles.ratingStars}>
                      {[1, 2, 3, 4, 5].map(n => (
                        <label key={n} className={pageStyles.ratingStar}>
                          <input type="radio" name="rating" value={n} defaultChecked={(mine?.rating ?? 0) === n} required />
                          <Star size={30} strokeWidth={1.5} aria-hidden="true" />
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
                <div className={pageStyles.signIn} data-sq="surface">
                  <p><strong>Used {app.name}?</strong> Sign in with Silicon Accounts to rate it and say what you think.</p>
                  <Action href={signInHref(`${path}#reviews`)} rel="nofollow">Sign in to write a review</Action>
                </div>
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
                      <article className={pageStyles.review} data-sq="surface" data-mine={review === mine ? "" : undefined} aria-label={review === mine ? "Your review" : `Review by ${review.id}`}>
                        <header className={pageStyles.reviewHead}>
                          <span className={pageStyles.reviewer}>
                            <Avatar name={review.id} id={review.id} size={30} />
                            <span className={pageStyles.reviewerId}>{review.id}</span>
                            <span className={review === mine ? `${styles.badge} ${styles.badgeAccent}` : styles.badge} data-sq="surface">{review === mine ? "You" : accountKind(review.id)}</span>
                          </span>
                          <time dateTime={review.updated_at} className={pageStyles.reviewDate}>{formatDate(review.updated_at)}</time>
                        </header>
                        <Stars rating={review.rating} />
                        {review.text ? <p className={pageStyles.reviewText}>{review.text}</p> : null}
                      </article>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className={pageStyles.noReviews}>No reviews yet. Be the first to say what you think.</p>
              )}
            </section>
          </div>

          <aside className={pageStyles.aside} aria-label={`More about ${app.name}`}>
            <section aria-labelledby="channels-title" className={pageStyles.card} data-sq="surface">
              <h2 id="channels-title" className={pageStyles.cardTitle}><Package {...ICON} aria-hidden="true" />Releases</h2>
              {production || development ? (
                <dl className={pageStyles.releases}>
                  {production ? (
                    <div>
                      <dt>Production</dt>
                      <dd>
                        <span className={pageStyles.version}>{production.version}</span> <span className={pageStyles.statNote}>{formatDate(production.created_at)}</span>
                        {production.notes ? <p>{truncate(production.notes, 220)}</p> : null}
                      </dd>
                    </div>
                  ) : null}
                  {development ? (
                    <div>
                      <dt>Development</dt>
                      <dd>
                        <span className={pageStyles.version}>{development.version}</span> <span className={pageStyles.statNote}>{formatDate(development.created_at)}</span>
                        {development.notes ? <p>{truncate(development.notes, 220)}</p> : null}
                      </dd>
                    </div>
                  ) : null}
                </dl>
              ) : (
                <p className={pageStyles.cardText}>No release yet. Check back soon.</p>
              )}
              {withdrawn.length ? (
                <div className={pageStyles.withdrawn}>
                  <h3 className={pageStyles.subTitle}>Withdrawn</h3>
                  <ul role="list">
                    {withdrawn.map(item => (
                      <li key={item.release_id}>
                        <p className={pageStyles.withdrawnHead}>
                          <span className={pageStyles.version}>{item.version}</span>
                          <span className={styles.badge} data-sq="surface">{item.channel === "production" ? "Production" : "Development"}</span>
                          <time dateTime={item.withdrawn_at} className={pageStyles.statNote}>{formatDate(item.withdrawn_at)}</time>
                        </p>
                        {item.reason ? <p className={pageStyles.withdrawnReason}>{item.reason}</p> : null}
                      </li>
                    ))}
                  </ul>
                  <p className={pageStyles.cardText}>A withdrawn release is never installed again, and updates move every copy of it to the latest good release.</p>
                </div>
              ) : null}
              {development || production ? (
                <details className={pageStyles.more}>
                  <summary>Other ways to install</summary>
                  <div className={pageStyles.moreBody}>
                    {development ? (
                      <div className={pageStyles.moreItem}>
                        <p>The latest development release</p>
                        <Command value={`silicon-apps install '${app.app_id}>dev'`} label="Copy the development install command" wrap />
                      </div>
                    ) : null}
                    {production ? (
                      <div className={pageStyles.moreItem}>
                        <p>This exact version</p>
                        <Command value={`silicon-apps install '${app.app_id}@${production.version}'`} label="Copy the exact version install command" wrap />
                      </div>
                    ) : null}
                    <p className={pageStyles.cardText}>The CLI asks before switching between production and development. An exact version picks what you install first; updates still follow its channel.</p>
                  </div>
                </details>
              ) : null}
            </section>

            {platforms.length ? (
              <section aria-labelledby="platforms-title" className={pageStyles.card} data-sq="surface">
                <h2 id="platforms-title" className={pageStyles.cardTitle}><Layers {...ICON} aria-hidden="true" />Available on</h2>
                <ul className={pageStyles.platforms} role="list">
                  {platforms.map(group => (
                    <li key={group.os}>
                      <span className={pageStyles.platformOs}>{group.os}</span>
                      <span className={pageStyles.platformArchs}>
                        {group.targets.map(target => (
                          <span key={target} className={styles.badge} data-sq="surface" title={target}>{targetLabel(target).replace(`${group.os} `, "")}</span>
                        ))}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}

            {released ? (
              <section aria-labelledby="signatures-title" className={pageStyles.card} data-sq="surface">
                <h2 id="signatures-title" className={pageStyles.cardTitle}><ShieldCheck {...ICON} aria-hidden="true" />Signatures</h2>
                <dl className={pageStyles.signatures}>
                  <div>
                    <dt>Silicon Apps</dt>
                    <dd data-state={app.signed ? "yes" : "no"}>{app.signed ? "Signed every package" : "Not signed yet"}</dd>
                  </div>
                  <div>
                    <dt>Authors</dt>
                    <dd data-state={signedBy.length ? "yes" : "no"}>{signedBy.length ? `Signed by ${signedBy.join(", ")}` : "No author signature"}</dd>
                  </div>
                </dl>
                <p className={pageStyles.cardText}>
                  For the current {production ? "production" : "development"} release. <a href={`${LINKS.appsDocs}/learn/signed-releases`}>How signed releases work</a>
                </p>
              </section>
            ) : null}

            {app.authors.length ? (
              <section aria-labelledby="authors-title" className={pageStyles.card} data-sq="surface">
                <h2 id="authors-title" className={pageStyles.cardTitle}><Users {...ICON} aria-hidden="true" />Authors</h2>
                <ul className={pageStyles.people} role="list">
                  {app.authors.map(author => (
                    <li key={author.uuid}>
                      <a href={authorPath(author.uuid)} className={pageStyles.person} data-sq="surface">
                        <Avatar name={author.display_name || author.id} id={author.id} size={36} />
                        <span className={pageStyles.personText}>
                          <span className={pageStyles.personName}>{author.display_name || author.id}</span>
                          <span className={pageStyles.personId}>{author.id}</span>
                        </span>
                      </a>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}

            {outbound.length ? (
              <section aria-labelledby="links-title" className={pageStyles.card} data-sq="surface">
                <h2 id="links-title" className={pageStyles.cardTitle}><Globe {...ICON} aria-hidden="true" />Around the web</h2>
                <ul className={pageStyles.links} role="list">
                  {outbound.map((link, index) => {
                    const Icon = LINK_ICONS[link.icon];
                    return (
                      <li key={`${link.url}-${index}`}>
                        <a href={link.url} rel="nofollow ugc noopener noreferrer" className={pageStyles.link} data-sq="surface">
                          {link.logo ? (
                            // eslint-disable-next-line @next/next/no-img-element
                            <img src={link.logo} alt="" width={18} height={18} className={pageStyles.linkLogo} />
                          ) : (
                            <Icon size={17} strokeWidth={1.75} aria-hidden="true" />
                          )}
                          <span>{link.label}</span>
                          <ArrowUpRight size={14} strokeWidth={1.75} aria-hidden="true" className={pageStyles.linkArrow} />
                        </a>
                      </li>
                    );
                  })}
                </ul>
              </section>
            ) : null}
          </aside>
        </div>
      </article>
    </div>
  );
}
