/**
 * /search?q=&tag=&target=&visibility=&page=: search and browse, server-rendered from a plain GET form. Search ranks
 * exact ids and names first (the Apps API's order); tag and platform narrow it down. The private view asks a signed-out
 * visitor to log in, and never shows a private app to anyone it was not shared with (the API decides).
 */
import type { Metadata } from "next";
import { Lock, Search as SearchIcon } from "lucide-react";
import { AppGrid } from "@/components/store/app-card";
import { Breadcrumbs, EmptyState, Notice, Pagination, StructuredData, styles } from "@/components/store/parts";
import { SearchFilters } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { listAll, parseSearch, search, tagCounts, type SearchInput, type SearchResult } from "@/lib/catalog";
import { targetLabel } from "@/lib/format";
import { absolute, breadcrumbLd, itemListLd, pageMetadata } from "@/lib/seo";
import { getAccount, signInHref } from "@/lib/session";

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

function heading(input: SearchInput): string {
  if (input.q) return `Apps matching “${input.q}”`;
  if (input.visibility === "private") return input.tag ? `Private apps tagged ${input.tag}` : "Private apps";
  if (input.tag && input.target) return `Apps tagged ${input.tag} for ${targetLabel(input.target)}`;
  if (input.tag) return `Apps tagged ${input.tag}`;
  if (input.target) return `Apps for ${targetLabel(input.target)}`;
  return "All apps";
}

function hrefFor(input: SearchInput, page = 1): string {
  const params = new URLSearchParams();
  if (input.q) params.set("q", input.q);
  if (input.tag) params.set("tag", input.tag);
  if (input.target) params.set("target", input.target);
  if (input.visibility !== "all") params.set("visibility", input.visibility);
  if (page > 1) params.set("page", String(page));
  const query = params.toString();
  return `/search${query ? `?${query}` : ""}`;
}

export async function generateMetadata({ searchParams }: Props): Promise<Metadata> {
  const input = parseSearch(await searchParams);
  const title = heading(input);
  const description = input.q
    ? `Apps on Silicon Apps matching ${input.q}, exact ids and names first. Every app installs with one command.`
    : input.tag
      ? `Apps on Silicon Apps tagged ${input.tag}: command line apps for Carbons and Silicons that install with one command and keep themselves up to date.`
      : input.target
        ? `Apps on Silicon Apps with a package for ${targetLabel(input.target)}. Every app installs with one command and keeps itself up to date.`
        : "Every app on Silicon Apps: command line apps for Carbons and Silicons that install with one command and keep themselves up to date.";
  // Typed searches and the private view are not indexed; the full list, tag pages and platform pages are.
  const index = !input.q && input.visibility === "all" && input.page === 1;
  return pageMetadata({ title, description, path: hrefFor({ ...input, q: "", visibility: "all", page: 1 }), index });
}

export default async function SearchPage({ searchParams }: Props) {
  const input = parseSearch(await searchParams);
  const account = await getAccount();
  const title = heading(input);
  const needsSignIn = input.visibility === "private" && !account;

  let result: SearchResult | null = null;
  let tags: string[] = [];
  let failed = false;
  if (!needsSignIn) {
    try {
      const [found, everything] = await Promise.all([search(input), listAll()]);
      result = found;
      tags = tagCounts(everything).map(entry => entry.tag);
    } catch {
      failed = true;
    }
  }
  const filtered = Boolean(input.q || input.tag || input.target || input.visibility !== "all");
  const path = hrefFor(input, input.page);

  return (
    <div className={styles.page}>
      <StructuredData
        graph={[
          breadcrumbLd([{ name: "Home", path: "/" }, { name: "All apps", path: "/search" }, ...(filtered ? [{ name: title, path: hrefFor(input) }] : [])]),
          {
            "@type": input.q ? "SearchResultsPage" : "CollectionPage",
            "@id": `${absolute(hrefFor(input))}#page`,
            name: title,
            url: absolute(hrefFor(input)),
            ...(result?.items.length ? { mainEntity: itemListLd(title, result.items) } : {}),
          },
        ]}
      />
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "All apps", href: "/search" }, ...(filtered ? [{ name: title }] : [])]} />
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

      {needsSignIn ? null : <SearchFilters input={input} tags={tags} />}

      <section aria-labelledby="results-title" className={styles.results}>
        <h2 id="results-title" className="sr-only">Results</h2>
        {needsSignIn ? (
          <EmptyState icon={<Lock size={20} strokeWidth={1.75} />} title="Log in to see private apps" actions={<Action href={signInHref(path)} rel="nofollow">Sign in with Silicon Accounts</Action>}>
            <p>Private apps show only to the Carbons and Silicons they are shared with. Sign in with your Silicon Accounts account to see the ones shared with you.</p>
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
            <AppGrid apps={result.items} label={title} />
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
