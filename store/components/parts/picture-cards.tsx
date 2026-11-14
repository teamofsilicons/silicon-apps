/**
 * Picture cards, the account site's (components/account/parts/picture-cards.tsx, Home's introduction there; copied
 * 2026-11-14): an h3 title, then three cards of one design in the page's column, each a picture on top (16:9, one of
 * the account site's Super Visuals pictures from public/banner, nothing on it) and a title with one line under it. A
 * card with `href` is a link as a whole and rises a little on hover.
 *
 * Server-rendered with plain links, so it works without script. The account site's dismiss button is left out: it needs
 * script and somewhere to remember the choice.
 */
import type { ReactNode } from "react";
import styles from "./picture-cards.module.css";

export interface PictureCard {
  id: string;
  /** The picture: a /banner/*.webp and where its focus is (CSS background-position). */
  picture: { src: string; focus?: string };
  title: string;
  line: ReactNode;
  /** Where the card goes: the whole card is the link. */
  href?: string;
}

/** `ratio`: the picture's shape, width / height. `layout`: "grid" shares the width between the cards; "scroll" gives
 * each a fixed, wider width in a row that scrolls sideways. */
export function PictureCards({ title, cards, ratio = "16 / 9", layout = "grid", className }: { title: string; cards: PictureCard[]; ratio?: string; layout?: "grid" | "scroll"; className?: string }) {
  return (
    <section className={className ? `${styles.banner} ${className}` : styles.banner} aria-label={title}>
      <div className={styles.head}>
        <h3 className={styles.title}>{title}</h3>
      </div>
      <ul className={styles.cards} data-layout={layout}>
        {cards.map(card => (
          <li key={card.id} data-sq="clip" className={styles.card} data-active={card.href ? "" : undefined}>
            {card.href ? <a href={card.href} className={styles.link}><Face card={card} ratio={ratio} /></a> : <Face card={card} ratio={ratio} />}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** A card's picture and words, inside its link or on its own. */
function Face({ card, ratio }: { card: PictureCard; ratio: string }) {
  return (
    <>
      <span className={styles.picture} style={{ backgroundImage: `url("${card.picture.src}")`, backgroundPosition: card.picture.focus ?? "center", aspectRatio: ratio }} aria-hidden="true" />
      <span className={styles.words}>
        <span className={styles.cardTitle}>{card.title}</span>
        <span className={styles.cardLine}>{card.line}</span>
      </span>
    </>
  );
}
