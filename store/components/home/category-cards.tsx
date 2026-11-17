/**
 * The home page's categories (Anil, 2026-11-14): an h3 title, then a card for each category with no box of its own:
 * its banner picture (public/categories, the corner and hairline of every picture here) with its name under
 * it, flush with the picture's edge; the whole card is one link to the category's page. Server-rendered with plain
 * links, so it works without script.
 */
import styles from "./category-cards.module.css";

export type CategoryCard = { id: string; title: string; picture: string; href: string };

export function CategoryCards({ title, cards }: { title: string; cards: CategoryCard[] }) {
  return (
    <section className={styles.section} aria-labelledby="category-cards-title">
      <h3 id="category-cards-title" className={styles.title}>{title}</h3>
      <ul className={styles.cards}>
        {cards.map(card => (
          <li key={card.id}>
            <a href={card.href} className={styles.card}>
              <span className={styles.picture} data-sq="clip" style={{ backgroundImage: `url("${card.picture}")` }} aria-hidden="true" />
              <span className={styles.name}>{card.title}</span>
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}
