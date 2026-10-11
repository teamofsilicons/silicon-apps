/**
 * Search, as plain GET forms to the home page (/): they work with no script, and every result page has its own URL. The field
 * is Arc's search field; the additional filters are native selects and a segmented control behind a disclosure.
 */
import { Search, SlidersHorizontal } from "lucide-react";
import { SearchToolbar } from "./search-toolbar";
import buttonStyles from "@/components/silicon-ui/button/button.module.css";
import { activeFilters, type SearchInput } from "@/lib/catalog";
import { MIN_INSTALLS, MIN_RATINGS, PERIOD_LABELS, SORT_LABELS, SORTS, installsLabel, ratingLabel } from "@/lib/filters";
import { TARGETS, targetLabel } from "@/lib/format";
import { styles } from "./parts";

export function SearchBox({ q = "", large = false, id = "search-q", label = "Search apps" }: { q?: string; large?: boolean; id?: string; label?: string }) {
  return (
    <form className={`${styles.search} ${large ? styles.searchLarge : ""}`} method="get" action="/" role="search" aria-label={label}>
      <label htmlFor={id} className="sr-only">{label}</label>
      <div className={styles.searchShell} data-sq="surface">
        <Search size={large ? 20 : 18} strokeWidth={1.75} aria-hidden="true" />
        <input id={id} name="q" type="search" className={styles.searchInput} defaultValue={q} placeholder="Search apps" autoComplete="off" spellCheck={false} enterKeyHint="search" maxLength={200} aria-keyshortcuts="/" />
        <kbd className={styles.shortcut} data-js-only="" aria-hidden="true">/</kbd>
        <button type="submit" className={`${buttonStyles.button} ${buttonStyles.primary} ${large ? buttonStyles.lg : buttonStyles.md} ${styles.searchButton}`}>Search</button>
      </div>
    </form>
  );
}

/**
 * The search page's one GET form: the search field, then the additional filters. With script they are Arc's filter
 * toolbar (components/store/search-toolbar.tsx): chips for what is set, Add filter, Clear all. Without script the same
 * filters are plain controls behind a native "Filters" disclosure, closed until one is set. Those controls stay in the
 * form either way (hidden with script), so a new search keeps the filters already set; the page tidies the address
 * after submitting (empty and default values are dropped).
 */
export function SearchFilters({ input }: { input: SearchInput }) {
  const active = activeFilters(input);
  const values: Record<string, string> = Object.fromEntries(
    Object.entries({
      q: input.q,
      target: input.target,
      visibility: input.visibility === "all" ? "" : input.visibility,
      added: input.added,
      released: input.released,
      installs: input.minInstalls ? String(input.minInstalls) : "",
      rating: input.minRating ? String(input.minRating) : "",
      sort: input.sort === "relevance" ? "" : input.sort,
    }).filter(([, value]) => value),
  );
  const select = (id: string, name: string, label: string, value: string | number, options: Array<[string, string]>) => (
    <div className={styles.field}>
      <label htmlFor={id} className={styles.label}>{label}</label>
      <select id={id} name={name} defaultValue={String(value || "")} className={styles.select} data-sq="surface">
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue}>{optionLabel}</option>
        ))}
      </select>
    </div>
  );
  return (
    <form className={styles.filters} method="get" action="/" role="search" aria-label="Search and filter apps">
      <label htmlFor="filter-q" className="sr-only">Search apps</label>
      <div className={styles.searchShell} data-sq="surface">
        <Search size={18} strokeWidth={1.75} aria-hidden="true" />
        <input id="filter-q" name="q" type="search" className={styles.searchInput} defaultValue={input.q} placeholder="Search apps" autoComplete="off" spellCheck={false} enterKeyHint="search" maxLength={200} aria-keyshortcuts="/" />
        <kbd className={styles.shortcut} data-js-only="" aria-hidden="true">/</kbd>
        <button type="submit" className={`${buttonStyles.button} ${buttonStyles.primary} ${buttonStyles.md} ${styles.searchButton}`}>Search</button>
      </div>
      <div data-js-only="" className={styles.toolbar}>
        <SearchToolbar values={values} />
      </div>
      <details className={styles.more} open={active > 0} data-nojs-only="">
        <summary className={styles.moreToggle}>
          <SlidersHorizontal size={14} strokeWidth={1.75} aria-hidden="true" />
          Filters{active ? <span className={styles.moreCount}>{active}<span className="sr-only"> set</span></span> : null}
        </summary>
        <div className={styles.moreBody}>
          {select("filter-target", "target", "Platform", input.target, [["", "Any platform"], ...TARGETS.map((target): [string, string] => [target, targetLabel(target)])])}
          {select("filter-sort", "sort", "Sort by", input.sort === "relevance" ? "" : input.sort, SORTS.map((sort): [string, string] => [sort === "relevance" ? "" : sort, SORT_LABELS[sort]]))}
          {select("filter-added", "added", "Added", input.added, [["", "Any time"], ...Object.entries(PERIOD_LABELS)])}
          {select("filter-released", "released", "Last release", input.released, [["", "Any time"], ...Object.entries(PERIOD_LABELS)])}
          {select("filter-installs", "installs", "Installs", input.minInstalls, [["", "Any"], ...MIN_INSTALLS.map((n): [string, string] => [String(n), installsLabel(n)])])}
          {select("filter-rating", "rating", "Rating", input.minRating, [["", "Any"], ...MIN_RATINGS.map((n): [string, string] => [String(n), ratingLabel(n)])])}
          <fieldset className={`${styles.segmented} ${styles.moreWide}`}>
            <legend className={styles.label}>Visibility</legend>
            <div className={styles.segmentTrack} data-sq="surface">
              {(["all", "public", "private"] as const).map(value => (
                <label key={value} className={styles.segment}>
                  <input type="radio" name="visibility" value={value} defaultChecked={input.visibility === value} />
                  <span data-sq="surface">{value === "all" ? "All" : value === "public" ? "Public" : "Private"}</span>
                </label>
              ))}
            </div>
          </fieldset>
          <div className={`${styles.moreActions} ${styles.moreWide}`}>
            {active ? <a href={input.q ? `/?q=${encodeURIComponent(input.q)}` : "/"} className={styles.clear}>Clear filters</a> : null}
            <button type="submit" className={`${buttonStyles.button} ${buttonStyles.secondary} ${buttonStyles.md}`}>Show apps</button>
          </div>
        </div>
      </details>
    </form>
  );
}
