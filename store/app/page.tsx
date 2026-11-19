/**
 * /: the store's home, designed again one element at a time inside the account site's layout (Anil, 2026-11-14):
 * - a picture-only banner for now (components/home/hero.tsx: one terminal window installing an app, on a soft gradient; the
 *   earlier banners, two pictures cross-fading and a wall of app icons, are in /playground), standing for Silicon Apps;
 * - "Core Apps to Try": three apps as the account site's picture cards (components/parts/picture-cards.tsx, its Home
 *   introduction's design), each with one of its pictures, the app's name and a short line, the whole card a link to
 *   the app. They are picked by hand (CORE_APPS: Briefcase, DM and Ring).
 * - "Categories" (components/home/category-cards.tsx, Anil, 2026-11-14): a card for each category with no box: its
 *   banner from its own page (public/categories), its name and an "Explore apps" button to it. Essentials, the account
 *   and the store every visitor already has, is left out, so six cards fill two rows.
 * The banner carousel (components/home/banner-carousel.tsx) is off the page for now (Anil, 2026-11-14), kept in
 * /playground in both its styles; the first layout's side-by-side feature (components/home/feature.tsx) is kept, unused.
 * A search (?q=, from the sidebar's search field) shows its results here instead: the matching apps as rows, 24 at a
 * time, after any category whose name or line matches.
 */
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { AppRows } from "@/components/home/all-apps";
import { limitWords, withoutName } from "@/components/home/feature";
import { CategoryCards, type CategoryCard } from "@/components/home/category-cards";
import { Hero } from "@/components/home/hero";
import styles from "@/components/home/home.module.css";
import { Page } from "@/components/parts/page";
import { PictureCards, type PictureCard } from "@/components/parts/picture-cards";
import { APP_PICTURES } from "@/lib/app-pictures";
import { listAll, parseSearch, search, type SearchResult } from "@/lib/catalog";
import { categorize } from "@/lib/categories";
import { appPath } from "@/lib/seo";
import type { App } from "@/lib/types";

/** The core apps (Anil, 2026-11-14): Briefcase, DM and Ring, picked by hand, in this order. */
const CORE_APPS = ["briefcase", "dm", "ring"];

/** For an app with no picture of its own: the account site's three introduction pictures, in its order. */
const PICTURES: PictureCard["picture"][] = [
  { src: "/banner/bg-110.webp" },
  { src: "/banner/bg-102.webp", focus: "right center" },
  { src: "/banner/bg-127.webp", focus: "center 70%" },
];

/** A card's line: the description's first sentence (without the app's name at its start where that reads well), at most
 * 15 words, so it ends on a full sentence for every app here and fits two lines. */
const LINE_WORDS = 15;

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

/** A search (?q=, from the sidebar's search field): the apps that match, the API's order (exact ids and names first,
 * then prefixes, then typos; tags matched through the text), 24 at a time with Show More, as the account site's rows,
 * after any category whose name or line matches. */
async function SearchResults({ params }: { params: Record<string, string | string[] | undefined> }) {
  const input = parseSearch(params);
  const result = await search(input).catch((): SearchResult | null => null);
  const apps = await listAll().catch((): App[] => []);
  const needle = input.q.toLowerCase();
  const categories = categorize(apps).filter(category => category.id !== "more" && `${category.title} ${category.line}`.toLowerCase().includes(needle));
  const more = new URLSearchParams({ q: input.q, page: String(input.page + 1) }).toString();
  return (
    <Page>
      <div className={styles.results}>
        <div className={styles.resultsHead}>
          <h1 className={styles.resultsTitle}>Apps matching “{input.q}”</h1>
          {result ? <p className={styles.resultsCount}>{result.total === 1 ? "1 app" : `${result.total} apps`}</p> : null}
        </div>
        {categories.length ? (
          <nav className={styles.resultsCategories} aria-label="Categories">
            {categories.map(category => <a key={category.id} href={`/categories/${category.id}`}>{category.title}</a>)}
          </nav>
        ) : null}
        {!result ? (
          <p className={styles.resultsNote}>The catalog is not answering right now. Try again in a moment, or run <code>silicon-apps search {input.q}</code> in your terminal.</p>
        ) : result.items.length ? (
          <>
            <AppRows apps={result.items} label={`Apps matching ${input.q}`} />
            {result.more ? <div className={styles.resultsMore}><ButtonLink href={`/?${more}`} variant="secondary">Show More Apps</ButtonLink></div> : null}
          </>
        ) : (
          <p className={styles.resultsNote}>No apps match that. Try a shorter name or another spelling, or <a href="/apps">browse every app</a>.</p>
        )}
      </div>
    </Page>
  );
}

export default async function Home({ searchParams }: Props) {
  const params = await searchParams;
  if (typeof params.q === "string" && params.q.trim()) return <SearchResults params={params} />;
  const apps = await listAll().catch((): App[] => []);
  // Picked by hand, in this order; one the visitor can't see (or that isn't in the catalog) is left out.
  const core = CORE_APPS.flatMap(id => apps.find(app => app.app_id === id) ?? []);
  const cards: PictureCard[] = core.map((app, index) => ({
    id: app.app_id,
    picture: APP_PICTURES[app.app_id] ?? PICTURES[index],
    title: app.name,
    line: app.description ? limitWords(withoutName(app.name, app.description), LINE_WORDS) : "No description yet.",
    href: appPath(app.app_id),
  }));
  const categories: CategoryCard[] = categorize(apps)
    .filter(category => category.id !== "essentials" && category.id !== "more")
    .map(category => ({
      id: category.id,
      title: category.title,
      picture: category.id === "ai-agents" ? "/categories/ai-agents.svg" : `/categories/${category.id}.webp`,
      href: `/categories/${category.id}`,
    }));
  return (
    <Page>
      <h1 className="sr-only">Silicon Apps</h1>
      <div className={styles.stack}>
        <Hero />
        {cards.length ? <PictureCards title="Core Apps to Try" cards={cards} /> : null}
        {categories.length ? <CategoryCards title="Categories" cards={categories} /> : null}
      </div>
    </Page>
  );
}
