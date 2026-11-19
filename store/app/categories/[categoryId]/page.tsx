/**
 * /categories/{id}: one category (Anil, 2026-11-14), the whole of it, for when a chapter on All Apps shows
 * only its first six: the page bar (All Apps above its title), a picture-only banner (Anil, 2026-11-14) with its line under it, then three
 * the results toolbar (Anil, 2026-11-14): the system as a dropdown on the left; the order as a dropdown and the view as two
 * icons on the right (dropdowns are a native popover of links on a button, so everything works without script, and
 * every view has its own address): the order (Popular, New, Top Rated: rated apps
 * with at least 3 reviews first), the system (All, macOS, Linux, Windows), and the view (Cards or List). Then its apps,
 * 24 at a time, with Show More for the next 24 (?page=N shows the first N batches). Server-rendered.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { LayoutGrid, List } from "lucide-react";
import { Gallery } from "@/components/gallery/gallery";
import { AppRows } from "@/components/home/all-apps";
import { ChoiceMenu } from "@/components/parts/choice-menu";
import { Page } from "@/components/parts/page";
import { PageBar } from "@/components/parts/page-bar";
import { listAll } from "@/lib/catalog";
import { arrange, CATEGORIES, CATEGORY_SORTS, CATEGORY_SYSTEMS, findCategory, type CategorySort } from "@/lib/categories";
import { pageMetadata } from "@/lib/seo";
import type { App } from "@/lib/types";
import styles from "./category.module.css";

type Props = { params: Promise<{ categoryId: string }>; searchParams: Promise<Record<string, string | string[] | undefined>> };

const BATCH = 24;

/** The two views, shown as icons side by side, named for readers and on hover. */
const VIEWS = [
  { value: "cards", label: "Grid view", Icon: LayoutGrid },
  { value: "list", label: "List view", Icon: List },
] as const;

/** Each category's banner (Anil, 2026-11-14): a picture from Cosmos in public/categories/{id}.webp, chosen by hand, or,
 * for AI and Agents, a soft gradient drawn in CSS (no picture). */
const GRADIENT = new Set(["ai-agents"]);

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { categoryId } = await params;
  const category = CATEGORIES.find(item => item.id === categoryId);
  if (!category) return { title: { absolute: "Category not found · Silicon Apps" }, robots: { index: false, follow: true } };
  return pageMetadata({ title: category.title, description: category.line, path: `/categories/${category.id}` });
}

const one = (value: string | string[] | undefined) => (Array.isArray(value) ? value[0] : value) ?? "";

export default async function CategoryPage({ params, searchParams }: Props) {
  const [{ categoryId }, query] = await Promise.all([params, searchParams]);
  const apps = await listAll().catch((): App[] => []);
  const category = findCategory(apps, categoryId);
  if (!category || category.id === "more") notFound();

  const sort: CategorySort = CATEGORY_SORTS.some(item => item.value === one(query.sort)) ? (one(query.sort) as CategorySort) : "popular";
  const system = CATEGORY_SYSTEMS.some(item => item.value === one(query.os)) ? one(query.os) : "";
  const view = one(query.view) === "list" ? "list" : "cards";
  const page = Math.max(1, Math.min(50, Math.floor(Number(one(query.page))) || 1));
  const arranged = arrange(category.apps, sort, system);
  const shown = arranged.slice(0, page * BATCH);

  /** This page's address with one choice changed; defaults are left out, and a change goes back to the first batch. */
  const hrefWith = (change: Partial<{ sort: string; os: string; view: string; page: number }>) => {
    const next = { sort, os: system, view, page: 1, ...change };
    const params = new URLSearchParams();
    if (next.sort !== "popular") params.set("sort", next.sort);
    if (next.os) params.set("os", next.os);
    if (next.view !== "cards") params.set("view", next.view);
    if (next.page > 1) params.set("page", String(next.page));
    const text = params.toString();
    return `/categories/${category.id}${text ? `?${text}` : ""}`;
  };

  /** One choice as a dropdown of links (components/parts/choice-menu.tsx). */
  const choice = (label: string, items: readonly { value: string; label: string }[], current: string, key: "sort" | "os" | "view", showLabel = true) => (
    <ChoiceMenu id={`choice-${key}`} label={label} showLabel={showLabel} options={items.map(item => ({ label: item.label, href: hrefWith({ [key]: item.value }), current: item.value === current }))} />
  );

  return (
    <div>
      <PageBar parent={{ label: "All Apps", href: "/apps" }} title={category.title} />
      <Page wide>
        <header className={styles.intro}>
          {GRADIENT.has(category.id) ? (
            <div className={`${styles.banner} ${styles.gradient}`} data-sq="surface" aria-hidden="true" />
          ) : (
            // eslint-disable-next-line @next/next/no-img-element -- a fixed picture from public/, already sized
            <img className={styles.banner} src={`/categories/${category.id}.webp`} alt="" data-sq="surface" />
          )}
          <p className={styles.line}>{category.line}</p>
        </header>
        {/* The usual results toolbar: the filter on the left; the order, then the view as two icons, on the right. */}
        <div className={styles.choices}>
          {choice("Platform", CATEGORY_SYSTEMS, system, "os", false)}
          <div className={styles.end}>
            {choice("Sort by", CATEGORY_SORTS, sort, "sort")}
            <nav className={styles.views} aria-label="Layout">
              {VIEWS.map(item => (
                <a key={item.value} href={hrefWith({ view: item.value })} className={styles.viewOption} aria-current={item.value === view ? "page" : undefined} aria-label={item.label} title={item.label}>
                  <item.Icon size={16} aria-hidden="true" />
                </a>
              ))}
            </nav>
          </div>
        </div>
        {shown.length ? (
          view === "list" ? <AppRows apps={shown} label={category.title} /> : <Gallery apps={shown} label={category.title} />
        ) : (
          <p className={styles.empty}>No {category.title} apps for {CATEGORY_SYSTEMS.find(item => item.value === system)?.label} yet. <a href={hrefWith({ os: "" })}>See all platforms</a></p>
        )}
        {arranged.length > shown.length ? (
          <div className={styles.more}>
            <ButtonLink href={hrefWith({ page: page + 1 })} variant="secondary">Show More Apps</ButtonLink>
          </div>
        ) : null}
      </Page>
    </div>
  );
}
