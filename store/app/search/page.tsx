/**
 * /search?q=&target=&visibility=&page=: search and browse, server-rendered from a plain GET form. Search ranks exact
 * ids and names first (the Apps API's order) and matches tags through the search text; the platform narrows it down.
 * Tags are never a filter of their own: an old /search?tag=x address moves to /search?q=x. The private view asks a
 * signed-out visitor to log in, and never shows a private app to anyone it was not shared with (the API decides).
 */
import type { Metadata } from "next";
import { permanentRedirect, redirect } from "next/navigation";
import { Lock, Search as SearchIcon } from "lucide-react";
import { AppList } from "@/components/store/app-card";
import { Breadcrumbs, EmptyState, Notice, Pagination, StructuredData, styles } from "@/components/store/parts";
import { SearchFilters } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { activeFilters, parseSearch, search, type SearchInput, type SearchResult } from "@/lib/catalog";
import { targetLabel } from "@/lib/format";
import { absolute, breadcrumbLd, itemListLd, pageMetadata } from "@/lib/seo";
import { getAccount, signInHref } from "@/lib/session";

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

function heading(input: SearchInput): string {
  if (input.q) return `Apps matching “${input.q}”`;
  if (input.visibility === "private") return "Private apps";
  if (input.target) return `Apps for ${targetLabel(input.target)}`;
  return "Apps";
}

function hrefFor(input: SearchInput, page = 1): string {
  const params = new URLSearchParams();
  if (input.q) params.set("q", input.q);
  if (input.target) params.set("target", input.target);
  if (input.visibility !== "all") params.set("visibility", input.visibility);
  if (input.added) params.set("added", input.added);
  if (input.released) params.set("released", input.released);
  if (input.minInstalls) params.set("installs", String(input.minInstalls));
  if (input.minRating) params.set("rating", String(input.minRating));
  if (input.sort !== "relevance") params.set("sort", input.sort);
  if (page > 1) params.set("page", String(page));
  const query = params.toString();
  return `/search${query ? `?${query}` : ""}`;
}

export async function generateMetadata({ searchParams }: Props): Promise<Metadata> {
  const input = parseSearch(await searchParams);
  const title = heading(input);
  const description = input.q
    ? `Apps on Silicon Apps matching ${input.q}, exact ids and names first. Every app installs with one command.`
    : input.target
      ? `Apps on Silicon Apps with a package for ${targetLabel(input.target)}. Every app installs with one command and keeps itself up to date.`
      : "Every app on Silicon Apps: command line apps for Carbons and Silicons that install with one command and keep themselves up to date.";
  // Typed searches, the private view and the additional filters are not indexed; the full list and platform pages are.
  const plain = { ...input, q: "", visibility: "all" as const, added: "" as const, released: "" as const, minInstalls: 0, minRating: 0, sort: "relevance" as const, page: 1 };
  const index = !input.q && input.page === 1 && activeFilters(input) === (input.target ? 1 : 0);
  return pageMetadata({ title, description, path: hrefFor(plain), index });
}

/** An old tag address (/search?tag=x) becomes a plain search for x, keeping the other parameters. */
function tagRedirect(params: Record<string, string | string[] | undefined>): string | null {
  const raw = params.tag;
  const tag = (Array.isArray(raw) ? raw[0] : raw)?.trim();
  if (!tag) return null;
  const input = parseSearch(params);
  return hrefFor({ ...input, q: [input.q, tag.slice(0, 60)].filter(Boolean).join(" "), page: 1 });
}

export default async function SearchPage({ searchParams }: Props) {
  const params = await searchParams;
  const moved = tagRedirect(params);
  if (moved) permanentRedirect(moved);
  const input = parseSearch(params);
  // The filters form submits every control, empty and default ones too; answer with the tidy address instead.
  const clean = hrefFor(input, input.page);
  const asked = new URLSearchParams(Object.entries(params).flatMap(([key, value]) => (value === undefined ? [] : [[key, Array.isArray(value) ? value[0] : value]]))).toString();
  if (`/search${asked ? `?${asked}` : ""}` !== clean) redirect(clean);
  const account = await getAccount();
  const title = heading(input);
  const needsSignIn = input.visibility === "private" && !account;

  let result: SearchResult | null = null;
  let failed = false;
  if (!needsSignIn) {
    try {
      result = await search(input);
    } catch {
      failed = true;
    }
  }
  const filtered = Boolean(input.q || activeFilters(input));
  const path = hrefFor(input, input.page);

  return (
    <div className={styles.page}>
      <StructuredData
        graph={[
          breadcrumbLd([{ name: "Home", path: "/" }, { name: "Apps", path: "/search" }, ...(filtered && title !== "Apps" ? [{ name: title, path: hrefFor(input) }] : [])]),
          {
            "@type": input.q ? "SearchResultsPage" : "CollectionPage",
            "@id": `${absolute(hrefFor(input))}#page`,
            name: title,
            url: absolute(hrefFor(input)),
            ...(result?.items.length ? { mainEntity: itemListLd(title, result.items) } : {}),
          },
        ]}
      />
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "Apps", href: "/search" }, ...(filtered && title !== "Apps" ? [{ name: title }] : [])]} />
      <header className={styles.pageHead}>
        <h1 className={styles.pageTitle}>{title}</h1>
        <p className={styles.lede}>
          {input.q
            ? "Search looks at app ids, names, descriptions and tags. Partial names and small spelling mistakes still match, and exact matches always come first."
            : input.visibility === "private"
              ? "Private apps only show to the Carbons and Silicons they are shared with, once they sign in."
              : "Every app here is a command line app for Carbons and Silicons. Each one installs with one command and keeps itself up to date."}
        </p>
      </header>

      {needsSignIn ? null : <SearchFilters input={input} />}

      <section aria-labelledby="results-title" className={styles.results}>
        <h2 id="results-title" className="sr-only">Results</h2>
        {needsSignIn ? (
          <EmptyState icon={<Lock size={20} strokeWidth={1.75} />} title="Log in to see private apps" actions={<Action href={signInHref(path)} rel="nofollow">Sign in with Silicon Accounts</Action>}>
            <p>Sign in with your Silicon Accounts account, and the private apps shared with you show up here.</p>
          </EmptyState>
        ) : failed ? (
          <Notice tone="danger" title="Search is not answering right now">
            <p>We could not reach the Apps service. Try again in a moment, or run <code>silicon-apps search{input.q ? ` ${input.q}` : ""}</code> in your terminal.</p>
          </Notice>
        ) : result && result.items.length ? (
          <>
            <p className={styles.resultsCount} role="status">
              {result.total === 1 ? "1 app" : `${result.total} apps`}
              {result.pages > 1 ? `, page ${result.page} of ${result.pages}` : ""}
            </p>
            <AppList apps={result.items} label={title} />
            <Pagination page={result.page} pages={result.pages} href={page => hrefFor(input, page)} label="Result pages" />
          </>
        ) : (
          <EmptyState
            icon={<SearchIcon size={20} strokeWidth={1.75} />}
            title={input.visibility === "private" ? "No private apps yet" : filtered ? "No apps match that" : "No apps here yet"}
            actions={filtered ? <Action href="/search" variant="secondary">Clear search and filters</Action> : undefined}
          >
            <p>
              {input.visibility === "private"
                ? "No private apps have been shared with your account yet. When an author shares one with you, it shows up here."
                : filtered
                  ? "Try a shorter name, another spelling, or fewer filters."
                  : "Apps show up here the moment their authors publish them."}
            </p>
          </EmptyState>
        )}
      </section>
    </div>
  );
}
