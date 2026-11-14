/**
 * The home page's categories (Anil, 2026-11-14): an h3 title, then a card for each category with no box of its own:
 * its banner picture (public/categories, the corner and hairline of every picture here), its name under it, flush
 * with the picture's edge, and a small "Explore apps" button to its page. The picture is a link to the same page.
 * Server-rendered with plain links, so it works without script.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import styles from "./category-cards.module.css";

export type CategoryCard = { id: string; title: string; picture: string; href: string };

export function CategoryCards({ title, cards }: { title: string; cards: CategoryCard[] }) {
  return (
    <section className={styles.section} aria-labelledby="category-cards-title">
      <h3 id="category-cards-title" className={styles.title}>{title}</h3>
      <ul className={styles.cards}>
        {cards.map(card => (
          <li key={card.id} className={styles.card}>
            <a href={card.href} className={styles.pictureLink} tabIndex={-1} aria-hidden="true">
              <span className={styles.picture} data-sq="clip" style={{ backgroundImage: `url("${card.picture}")` }} />
            </a>
            <div className={styles.foot}>
              <span className={styles.name}>{card.title}</span>
              <ButtonLink href={card.href} variant="secondary" size="sm" aria-label={`Explore ${card.title} apps`}>Explore apps</ButtonLink>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
