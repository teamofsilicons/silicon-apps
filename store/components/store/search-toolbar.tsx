"use client";
/**
 * The search page's filters, kept quiet: one row under the search field with no box around it. Each filter is Arc's
 * dropdown menu (components/silicon-ui/dropdown-menu) with a plain trigger ("Platform"), and one click shows its values;
 * once set, the trigger reads its value ("Linux x64") and sits on a soft fill. Sort lives at the end of the row, and
 * "Clear" appears only when something is set. Each choice moves to the matching address on the home page, so the server renders
 * the results and every view keeps its own URL. Only shown with script; without it the page keeps its plain form.
 */
import { useRouter } from "next/navigation";
import { useEffect, useState, useTransition } from "react";
import { Check } from "lucide-react";
import { DropdownMenu, type DropdownItem } from "@/components/silicon-ui/dropdown-menu/dropdown-menu";
import { FILTERS, SORT_LABELS, SORTS, type Sort } from "@/lib/filters";
import { RatingFilter } from "./rating-filter";
import styles from "./search-toolbar.module.css";

/** The filters the row offers, in order, with the words for "any". Added and visibility stay in the URL and the plain form. */
const SHOWN: Array<{ param: string; any: string }> = [
  { param: "target", any: "Any platform" },
  { param: "rating", any: "Any rating" },
  { param: "installs", any: "Any number of installs" },
  { param: "released", any: "Any time" },
];

/** The order the home page writes its parameters in (app/page.tsx), so a move never needs the tidying redirect. */
const ORDER = ["q", "target", "visibility", "added", "released", "installs", "rating", "sort"];

function hrefOf(values: Record<string, string>): string {
  const params = new URLSearchParams();
  for (const key of ORDER) if (values[key]) params.set(key, values[key]);
  const query = params.toString();
  return `/${query ? `?${query}` : ""}`;
}

const mark = (on: boolean) => (on ? <Check size={15} strokeWidth={2} /> : <span />);

/** `values` holds the search text and every set filter, by URL parameter (unset ones are absent). */
export function SearchToolbar({ values }: { values: Record<string, string> }) {
  const router = useRouter();
  const [current, setCurrent] = useState(values);
  const [pending, startTransition] = useTransition();
  // While the next results load, the page dims them (styles/base.css: :root[data-busy] [data-results]).
  useEffect(() => {
    document.documentElement.toggleAttribute("data-busy", pending);
    return () => document.documentElement.removeAttribute("data-busy");
  }, [pending]);
  const go = (next: Record<string, string>) => {
    setCurrent(next);
    startTransition(() => router.push(hrefOf(next), { scroll: false }));
  };
  const set = (param: string, value: string) => {
    const next = { ...current };
    if (value) next[param] = value;
    else delete next[param];
    go(next);
  };
  const sort = (current.sort as Sort | undefined) ?? "relevance";
  const anySet = SHOWN.some(({ param }) => current[param]) || Boolean(current.added || current.visibility);

  return (
    <div className={styles.bar} role="group" aria-label="Filters">
      <div className={styles.filters}>
        {SHOWN.map(({ param, any }) => {
          if (param === "rating") return <RatingFilter key={param} value={Number(current.rating) || 0} onChange={value => set("rating", value ? String(value) : "")} />;
          const spec = FILTERS.find(entry => entry.param === param)!;
          const chosen = spec.options.find(option => option.value === current[param]);
          const items: DropdownItem[] = [
            { label: any, icon: mark(!chosen), onSelect: () => set(param, "") },
            ...spec.options.map((option, index) => ({ label: option.label, icon: mark(chosen?.value === option.value), onSelect: () => set(param, option.value), separatorBefore: index === 0 })),
          ];
          return (
            <span key={param} className={styles.filter} data-active={chosen ? "" : undefined}>
              <DropdownMenu label={chosen ? chosen.label : spec.label} items={items} />
            </span>
          );
        })}
        {anySet ? (
          <button type="button" className={styles.clear} onClick={() => go({ ...(current.q ? { q: current.q } : {}), ...(current.sort ? { sort: current.sort } : {}) })}>Clear</button>
        ) : null}
      </div>
      <span className={styles.filter} data-sort="">
        <DropdownMenu
          label={`Sort: ${SORT_LABELS[sort]}`}
          items={SORTS.map(option => ({ label: SORT_LABELS[option], icon: mark(option === sort), onSelect: () => set("sort", option === "relevance" ? "" : option) }))}
        />
      </span>
    </div>
  );
}
