/**
 * Search, as plain GET forms to /search: they work with no script, and every result page has its own URL. The field
 * is Arc's search field; the filters are Arc's select and segmented control, as native controls.
 */
import { Search } from "lucide-react";
import buttonStyles from "@/components/arc/button/button.module.css";
import type { SearchInput } from "@/lib/catalog";
import { TARGETS, targetLabel } from "@/lib/format";
import { styles } from "./parts";

export function SearchBox({ q = "", large = false, id = "search-q", label = "Search apps" }: { q?: string; large?: boolean; id?: string; label?: string }) {
  return (
    <form className={`${styles.search} ${large ? styles.searchLarge : ""}`} method="get" action="/search" role="search" aria-label={label}>
      <label htmlFor={id} className="sr-only">{label}</label>
      <div className={styles.searchShell} data-sq="surface">
        <Search size={large ? 20 : 18} strokeWidth={1.75} aria-hidden="true" />
        <input id={id} name="q" type="search" className={styles.searchInput} defaultValue={q} placeholder="Search apps" autoComplete="off" spellCheck={false} enterKeyHint="search" maxLength={200} />
        <button type="submit" className={`${buttonStyles.button} ${buttonStyles.primary} ${large ? buttonStyles.lg : buttonStyles.md} ${styles.searchButton}`} data-sq="surface">Search</button>
      </div>
    </form>
  );
}

export function SearchFilters({ input, tags }: { input: SearchInput; tags: string[] }) {
  const tagOptions = input.tag && !tags.includes(input.tag) ? [input.tag, ...tags] : tags;
  return (
    <form className={styles.filters} data-sq="surface" method="get" action="/search" role="search" aria-label="Search and filter apps">
      <div className={styles.field}>
        <label htmlFor="filter-q" className={styles.label}>Search</label>
        <div className={styles.searchShell} data-sq="surface">
          <Search size={18} strokeWidth={1.75} aria-hidden="true" />
          <input id="filter-q" name="q" type="search" className={styles.searchInput} defaultValue={input.q} placeholder="Name, id, tag or what it does" autoComplete="off" spellCheck={false} enterKeyHint="search" maxLength={200} />
        </div>
      </div>
      <div className={styles.filterRow}>
        <div className={styles.field}>
          <label htmlFor="filter-tag" className={styles.label}>Tag</label>
          <select id="filter-tag" name="tag" defaultValue={input.tag} className={styles.select} data-sq="surface">
            <option value="">Any tag</option>
            {tagOptions.map(tag => (
              <option key={tag} value={tag}>{tag}</option>
            ))}
          </select>
        </div>
        <div className={styles.field}>
          <label htmlFor="filter-target" className={styles.label}>Platform</label>
          <select id="filter-target" name="target" defaultValue={input.target} className={styles.select} data-sq="surface">
            <option value="">Any platform</option>
            {TARGETS.map(target => (
              <option key={target} value={target}>{targetLabel(target)}</option>
            ))}
          </select>
        </div>
        <fieldset className={styles.segmented}>
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
        <button type="submit" className={`${buttonStyles.button} ${buttonStyles.primary} ${buttonStyles.md} ${styles.filterSubmit}`} data-sq="surface">Show apps</button>
      </div>
    </form>
  );
}
