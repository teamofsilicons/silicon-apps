/**
 * An app page's Ratings and reviews (Anil, 2026-11-18), last, after Developer:
 * - its title (h2); then, where anyone has rated it, the average large with "out of 5" and how many ratings, beside the
 *   spread by stars (a bar for each of 5 to 1);
 * - after a write, a notice: saved, removed, or what went wrong (?review=saved|removed|error&code=…);
 * - signed out, "Used X? Sign in to write a review" (back to this section); signed in, one row, "Write a review" (or
 *   "Edit your review"), that opens into the form (a server action, app/apps/[appId]/actions.ts, so it works without
 *   script): five stars to pick, the words (600 characters at most, how many are left under them), Post Review or
 *   Save Changes, and "Remove your review" (a details) under it where they have one;
 * - the reviews as cards in columns (one on a phone), theirs first (marked You), then newest first: five stars, how
 *   long ago (the date on hover), the words (at most 600 characters), then who wrote it, by id. Six show; the rest are
 *   behind "N more reviews" (a details, so no script).
 * GET /v1/apps/{id}/reviews; a review carries the reviewer's id only, no name or picture. Server-rendered.
 */
import { SquarePen, Star, Triangle } from "lucide-react";
import { Alert } from "@teamofsilicons/silicon-ui/alert";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { removeReview, saveReview } from "@/app/apps/[appId]/actions";
import { ReviewText } from "@/components/app/review-text";
import { formatDate, formatRating, plural } from "@/lib/format";
import { ago } from "@/lib/releases";
import { signInHref } from "@/lib/session";
import type { Account, App, Review, Reviews as ReviewList } from "@/lib/types";
import styles from "./reviews.module.css";

const SHOWN = 6;

const ERRORS: Record<string, string> = {
  rating: "Choose a rating from 1 to 5 stars.",
  too_long: "Keep your review to 600 characters or fewer.",
  sign_in: "Your session has ended. Sign in again, then save your review.",
  unavailable: "We could not reach the Apps service. Nothing was saved; try again in a moment.",
  rate_limited: "Too many requests right now. Wait a minute, then save your review again.",
};

/** What came back from a write, read from the page's query (?review=saved|removed|error and &code=). */
export type ReviewStatus = { review?: string; code?: string };

/** Five stars, the first `rating` filled, read as "4 out of 5 stars". */
function Stars({ rating, size = 14 }: { rating: number; size?: number }) {
  return (
    <span className={styles.stars} role="img" aria-label={`${rating.toFixed(1)} out of 5 stars`}>
      {[1, 2, 3, 4, 5].map(star => (
        <Star key={star} size={size} strokeWidth={1.75} aria-hidden="true" className={styles.star} data-on={star <= Math.round(rating) ? "" : undefined} fill="currentColor" />
      ))}
    </span>
  );
}

function Card({ review, mine = false }: { review: Review; mine?: boolean }) {
  return (
    <li className={styles.card} data-mine={mine ? "" : undefined}>
      <article className={styles.article} aria-label={mine ? "Your review" : `Review by ${review.id}`}>
      <div className={styles.cardHead}>
        <Stars rating={review.rating} />
        <time dateTime={review.updated_at} title={formatDate(review.updated_at)}>{ago(review.updated_at)}</time>
      </div>
      {review.text.trim() ? <p className={styles.text}>{review.text.trim()}</p> : <p className={styles.quiet}>A rating without words.</p>}
      <p className={styles.by}>{review.id}{mine ? <span className={styles.you}>You</span> : null}</p>
      </article>
    </li>
  );
}

/** The form that writes (or edits) the visitor's one review, folded to one row until it's wanted (Anil, 2026-11-18: "no
 * point having such a big element"): "Write a review" (or "Edit your review") with a pencil and the triangle; open, the
 * stars, the words and the button, and, where they have one, "Remove your review" under them. It opens by itself after
 * an error. `preview` draws it without actions, for the playground, which reads the live catalog and must never write. */
function Compose({ app, account, mine, preview, open }: { app: App; account: Account; mine?: Review; preview: boolean; open: boolean }) {
  return (
    <details className={styles.compose} open={open}>
      <summary>
        <SquarePen className={styles.pen} size={16} strokeWidth={2} aria-hidden="true" />
        <h3 className={styles.composeTitle}>{mine ? "Edit your review" : "Write a review"}</h3>
        <span className={styles.composeHint}>As {account.id}</span>
        <Triangle className={styles.triangle} size={10} strokeWidth={2} fill="currentColor" aria-hidden="true" />
      </summary>
      <div className={styles.composeBody}>
        <form action={preview ? undefined : saveReview.bind(null, app.app_id, crypto.randomUUID())} className={styles.form} aria-label={mine ? "Edit your review" : `Review ${app.name}`}>
          <fieldset className={styles.pick}>
            <legend className={styles.legend}>Your rating</legend>
            <div className={styles.pickStars}>
              {[1, 2, 3, 4, 5].map(n => (
                <label key={n} className={styles.pickStar}>
                  {/* eslint-disable-next-line no-restricted-syntax -- Silicon UI has no star rating; native radios, drawn over each star with no opacity, keep it a plain form that works without script */}
                  <input type="radio" name="rating" value={n} defaultChecked={mine?.rating === n} required className={styles.radio} />
                  <Star size={28} strokeWidth={1.5} fill="currentColor" aria-hidden="true" />
                  <span className="sr-only">{n === 1 ? "1 star" : `${n} stars`}</span>
                </label>
              ))}
            </div>
          </fieldset>
          <ReviewText initial={mine?.text ?? ""} />
          <div className={styles.formActions}>
            <Button type="submit" size="sm" disabled={preview}>{mine ? "Save Changes" : "Post Review"}</Button>
            <span className={styles.quiet}>{preview ? "A preview: this form doesn’t post." : "One review per app; change or remove it any time."}</span>
          </div>
        </form>
        {mine ? (
          <details className={styles.remove}>
            <summary>Remove your review</summary>
            <form action={preview ? undefined : removeReview.bind(null, app.app_id, crypto.randomUUID())} className={styles.removeForm}>
              <p>This deletes your rating and words for {app.name}. You can write a new review later.</p>
              <Button type="submit" variant="danger" size="sm" disabled={preview}>Remove My Review</Button>
            </form>
          </details>
        ) : null}
      </div>
    </details>
  );
}

export function Reviews({ app, reviews, account, status = {}, preview = false }: { app: App; reviews: ReviewList; account: Account | null; status?: ReviewStatus; preview?: boolean }) {
  const mine = account ? reviews.items.find(review => review.uuid === account.uuid) : undefined;
  const sorted = [...reviews.items].filter(review => review !== mine).sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  if (mine) sorted.unshift(mine);
  const rated = reviews.count > 0 && reviews.rating !== null;
  const spread = [5, 4, 3, 2, 1].map(stars => ({ stars, count: reviews.items.filter(review => review.rating === stars).length }));
  const [shown, rest] = [sorted.slice(0, SHOWN), sorted.slice(SHOWN)];

  return (
    <section className={styles.section} id="reviews" aria-labelledby="reviews-title">
      <h2 id="reviews-title" className={styles.title}>Ratings and reviews</h2>

      {rated ? (
        <div className={styles.summary}>
          <div className={styles.average}>
            <p className={styles.score}>{formatRating(reviews.rating)}</p>
            <Stars rating={reviews.rating ?? 0} size={16} />
            <p className={styles.quiet}>out of 5 · {plural(reviews.count, "rating")}</p>
          </div>
          <ul className={styles.bars} aria-label="Ratings by stars">
            {spread.map(row => (
              <li key={row.stars}>
                <span className={styles.barLabel}>{row.stars}<span className="sr-only"> stars</span></span>
                <span className={styles.bar} aria-hidden="true"><span style={{ width: `${reviews.count ? (row.count / reviews.count) * 100 : 0}%` }} /></span>
                <span className={styles.barCount}>{row.count}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className={styles.quiet}>No ratings yet.</p>
      )}

      {status.review === "saved" ? <Alert tone="success" title="Your review is saved.">Thank you for sharing it.</Alert> : null}
      {status.review === "removed" ? <Alert tone="success" title="Your review is removed.">You can write a new one any time.</Alert> : null}
      {status.review === "error" ? <Alert tone="danger" title="Nothing was saved">{ERRORS[status.code ?? ""] ?? "We could not save your review. Check it and try again."}</Alert> : null}

      {account ? (
        <Compose app={app} account={account} mine={mine} preview={preview} open={status.review === "error"} />
      ) : (
        <p className={styles.signIn}>Used {app.name}? <a href={signInHref(`/apps/${encodeURIComponent(app.app_id)}#reviews`)} rel="nofollow">Sign in to write a review</a></p>
      )}

      {shown.length ? <ul className={styles.cards} aria-label="Reviews">{shown.map(review => <Card key={review.uuid} review={review} mine={review === mine} />)}</ul> : null}
      {rest.length ? (
        <details className={styles.more}>
          <summary>
            <Triangle className={styles.triangle} size={10} strokeWidth={2} fill="currentColor" aria-hidden="true" />
            {plural(rest.length, "more review")}
          </summary>
          <ul className={styles.cards} aria-label="More reviews">{rest.map(review => <Card key={review.uuid} review={review} />)}</ul>
        </details>
      ) : null}
    </section>
  );
}
