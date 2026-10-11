"use client";
/**
 * The search page's additional filters as Arc's filter toolbar (components/silicon-ui/filter-toolbar): every active
 * filter is a chip you can remove, "Add filter" drills into a field and its values, and "Clear all" resets them. Each
 * change moves to the matching /search address, so the server renders the results and every view keeps its own URL.
 * Only shown with script; without it the page keeps its plain form (components/store/search-form.tsx).
 */
import { useRouter } from "next/navigation";
import { useEffect, useState, useTransition, type ReactNode } from "react";
import { ArrowUpDown, CalendarPlus, Download, Eye, Laptop, Rocket, Star } from "lucide-react";
import { FilterToolbar, type FilterChip, type FilterField } from "@/components/silicon-ui/filter-toolbar/filter-toolbar";
import { FILTERS } from "@/lib/filters";

const ICONS: Record<string, ReactNode> = {
  target: <Laptop size={15} strokeWidth={1.75} aria-hidden="true" />,
  sort: <ArrowUpDown size={15} strokeWidth={1.75} aria-hidden="true" />,
  added: <CalendarPlus size={15} strokeWidth={1.75} aria-hidden="true" />,
  released: <Rocket size={15} strokeWidth={1.75} aria-hidden="true" />,
  installs: <Download size={15} strokeWidth={1.75} aria-hidden="true" />,
  rating: <Star size={15} strokeWidth={1.75} aria-hidden="true" />,
  visibility: <Eye size={15} strokeWidth={1.75} aria-hidden="true" />,
};

/** The order /search writes its parameters in (app/search/page.tsx), so a move never needs the tidying redirect. */
const ORDER = ["q", "target", "visibility", "added", "released", "installs", "rating", "sort"];

function hrefOf(values: Record<string, string>): string {
  const params = new URLSearchParams();
  for (const key of ORDER) if (values[key]) params.set(key, values[key]);
  const query = params.toString();
  return `/search${query ? `?${query}` : ""}`;
}

const FIELDS: FilterField[] = FILTERS.map(spec => ({ id: spec.param, label: spec.label, icon: ICONS[spec.param], options: spec.options.map(option => ({ value: option.value, label: option.label })) }));

function chipsOf(values: Record<string, string>): FilterChip[] {
  return FILTERS.flatMap(spec => {
    const option = spec.options.find(entry => entry.value === values[spec.param]);
    return option ? [{ id: spec.param, label: spec.label, value: option.label }] : [];
  });
}

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
  return (
    <FilterToolbar
      label="Filters"
      filters={chipsOf(current)}
      onRemove={id => {
        const next = { ...current };
        delete next[id];
        go(next);
      }}
      onClearAll={() => go(current.q ? { q: current.q } : {})}
      addFilter={{
        fields: FIELDS,
        label: "Add filter",
        align: "start",
        onAdd: (chip, field) => {
          const spec = FILTERS.find(entry => entry.param === field.id);
          const option = spec?.options.find(entry => entry.label === chip.value);
          if (option) go({ ...current, [field.id]: option.value });
        },
      }}
    />
  );
}
