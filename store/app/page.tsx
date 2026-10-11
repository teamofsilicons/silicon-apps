/**
 * /: the store, on one page. A serif title, the search field and the quiet filter row, then the apps in batches of
 * PAGE_SIZE ("Show more apps" adds the next one in place; ?page=N shows the first N batches). Search ranks exact ids
 * and names first (the Apps API's order) and matches tags through the search text; the filters narrow it. On the plain
 * view, how to get the CLI and what a Silicon can read here follow the list. The private view asks a signed-out visitor
 * to log in and never shows a private app to anyone it was not shared with (the API decides). Old addresses move here:
 * /search?… (proxy.ts) and /?tag=x, which becomes a search for x. Server-rendered; the filters, copy buttons and "Show
 * more" are the only script.
 */
import type { Metadata } from "next";
import { permanentRedirect, redirect } from "next/navigation";
import { Lock, Search as SearchIcon } from "lucide-react";
import { AppList } from "@/components/store/app-card";
import styles from "@/components/store/home.module.css";
import { LoadMore } from "@/components/store/load-more";
import { EmptyState, Notice, StructuredData } from "@/components/store/parts";
import { SearchFilters } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { CodeBlock } from "@/components/site/code-block";
import { activeFilters, PAGE_SIZE, parseSearch, search, type SearchInput, type SearchResult } from "@/lib/catalog";
import { targetLabel } from "@/lib/format";
import { absolute, itemListLd, pageMetadata } from "@/lib/seo";
import { getAccount, signInHref } from "@/lib/session";
import { CANONICAL_ORIGIN, INSTALL_UNIX, INSTALL_WINDOWS, LINKS, SITE_DESCRIPTION } from "@/lib/site";

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

const TITLE = "Silicon Apps: apps for Carbons and Silicons";

function heading(input: SearchInput): string {
  if (input.q) return `Apps matching “${input.q}”`;
  if (input.visibility === "private") return "Private apps";
  if (input.target) return `Apps for ${targetLabel(input.target)}`;
  return "Apps for Carbons and Silicons.";
}

/** The page's own address for a search, its parameters always in this order (the filter row writes the same). */
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
  return `/${query ? `?${query}` : ""}`;
}

export async function generateMetadata({ searchParams }: Props): Promise<Metadata> {
  const input = parseSearch(await searchParams);
  const plainView = !input.q && activeFilters(input) === 0;
  if (plainView) return pageMetadata({ title: TITLE, absoluteTitle: true, description: SITE_DESCRIPTION, path: "/" });
  const description = input.q
    ? `Apps on Silicon Apps matching ${input.q}, exact ids and names first. Every app installs with one command.`
    : input.target
      ? `Apps on Silicon Apps with a package for ${targetLabel(input.target)}. Every app installs with one command and keeps itself up to date.`
      : SITE_DESCRIPTION;
  // Typed searches, the private view and the other filters are not indexed; the plain list and platform lists are.
  const plain = { ...input, q: "", visibility: "all" as const, added: "" as const, released: "" as const, minInstalls: 0, minRating: 0, sort: "relevance" as const, page: 1 };
  const index = !input.q && input.page === 1 && activeFilters(input) === (input.target ? 1 : 0);
  return pageMetadata({ title: heading(input).replace(/\.$/, ""), description, path: hrefFor(plain), index });
}

/** An old tag address (?tag=x) becomes a plain search for x, keeping the other parameters. */
function tagRedirect(params: Record<string, string | string[] | undefined>): string | null {
  const raw = params.tag;
  const tag = (Array.isArray(raw) ? raw[0] : raw)?.trim();
  if (!tag) return null;
  const input = parseSearch(params);
  return hrefFor({ ...input, q: [input.q, tag.slice(0, 60)].filter(Boolean).join(" "), page: 1 });
}

export default async function Home({ searchParams }: Props) {
  const params = await searchParams;
  const moved = tagRedirect(params);
  if (moved) permanentRedirect(moved);
  const input = parseSearch(params);
  // The plain form submits every control, empty and default ones too; answer with the tidy address instead.
  const clean = hrefFor(input, input.page);
  const asked = new URLSearchParams(Object.entries(params).flatMap(([key, value]) => (value === undefined ? [] : [[key, Array.isArray(value) ? value[0] : value]]))).toString();
  if (`/${asked ? `?${asked}` : ""}` !== clean) redirect(clean);

  const account = await getAccount();
  const needsSignIn = input.visibility === "private" && !account;
  const filtered = Boolean(input.q || activeFilters(input));
  const title = heading(input);

  let result: SearchResult | null = null;
  let failed = false;
  if (!needsSignIn) {
    try {
      result = await search(input);
    } catch {
      failed = true;
    }
  }

  return (
    <div className={styles.page}>
      <StructuredData
        graph={[
          {
            "@type": input.q ? "SearchResultsPage" : filtered ? "CollectionPage" : "WebPage",
            "@id": `${absolute(hrefFor(input))}#webpage`,
            url: absolute(hrefFor(input)),
            name: filtered ? title : TITLE,
            description: SITE_DESCRIPTION,
            isPartOf: { "@id": `${CANONICAL_ORIGIN}/#website` },
          },
          ...(result?.items.length ? [itemListLd(filtered ? title : "Apps on Silicon Apps", result.items.filter(app => app.visibility === "public").slice(0, 30))] : []),
        ]}
      />

      <header className={styles.hero}>
        <p className={styles.kicker}>Silicon Apps</p>
        <h1 className={styles.title}>{title}</h1>
        {filtered ? null : (
          <p className={styles.lede}>
            Every app here is a command line app first, so a Silicon can install it and find its way around on its own. One
            command installs it, and it keeps itself up to date.
          </p>
        )}
      </header>

      {needsSignIn ? null : <div className={styles.searchArea}><SearchFilters input={input} /></div>}

      <section aria-labelledby="results-title" className={styles.results} data-results="">
        <h2 id="results-title" className="sr-only">{filtered ? "Results" : "Apps"}</h2>
        {needsSignIn ? (
          <EmptyState icon={<Lock size={20} strokeWidth={1.75} />} title="Log in to see private apps" actions={<Action href={signInHref(clean)} rel="nofollow">Sign in with Silicon Accounts</Action>}>
            <p>Sign in with your Silicon Accounts account, and the private apps shared with you show up here.</p>
          </EmptyState>
        ) : failed ? (
          <Notice tone="danger" title="The catalog is not answering right now">
            <p>We could not reach the Apps service. Try again in a moment, or run <code>silicon-apps search{input.q ? ` ${input.q}` : ""}</code> in your terminal.</p>
          </Notice>
        ) : result && result.items.length ? (
          <>
            {/* The count is for screen readers; the list speaks for itself on screen. */}
            <p className="sr-only" role="status">{result.total === 1 ? "1 app" : `${result.total} apps`}</p>
            <AppList apps={result.items} label={filtered ? title : "Apps"} />
            {result.more ? (
              <div className={styles.more}>
                <LoadMore href={hrefFor(input, input.page + 1)} remaining={Math.min(PAGE_SIZE, result.total - result.items.length)} />
              </div>
            ) : null}
          </>
        ) : (
          <EmptyState
            icon={<SearchIcon size={20} strokeWidth={1.75} />}
            title={input.visibility === "private" ? "No private apps yet" : filtered ? "No apps match that" : "No apps here yet"}
            actions={filtered ? <Action href="/" variant="secondary">Clear search and filters</Action> : <Action href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make the first Silicon App</Action>}
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

      {filtered || needsSignIn ? null : (
        <>
          <section className={styles.section} id="get-the-cli" aria-labelledby="cli-title">
            <div className={styles.head}>
              <h2 id="cli-title" className={styles.heading}>Get the CLI</h2>
            </div>
            <p className={styles.text}>
              Install <code>silicon-apps</code> once and every app is one command away. It checks every package before it runs
              and keeps itself, and every app you install, up to date. No account needed for public apps.
            </p>
            <CodeBlock code={INSTALL_UNIX} lang="sh" title="macOS and Linux" wrap />
            <details className={styles.windows}>
              <summary>On Windows</summary>
              <CodeBlock code={INSTALL_WINDOWS} lang="powershell" title="Windows PowerShell" wrap />
            </details>
            <p className={styles.note}>
              Then try <code>silicon-apps search notes</code> and <code>silicon-apps install briefcase</code>.
            </p>
          </section>

          <section className={styles.section} id="for-silicons" aria-labelledby="silicons-title">
            <div className={styles.head}>
              <h2 id="silicons-title" className={styles.heading}>For Silicons</h2>
            </div>
            <p className={styles.text}>
              Everything here is plain HTML and an API, so you never need a browser. Read the store in plain words at{" "}
              <a href="/llms.txt">/llms.txt</a> (all of it at <a href="/llms-full.txt">/llms-full.txt</a>), call the Apps API
              at <code>/v1</code> (spec at <a href="/openapi.json">/openapi.json</a>), or start from the{" "}
              <a href="/.well-known/agent.json">agent card</a>. Every app answers <code>--help</code>,{" "}
              <code>accounts --json</code> and <code>login status --json</code> on every system.
            </p>
          </section>

          <p className={styles.closing}>
            Made something useful? <a href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make a Silicon App</a> or{" "}
            <a href={LINKS.accountsDocs} target="_blank" rel="noopener noreferrer">add Silicon Accounts to your app</a> on Silicon Developer.
          </p>
        </>
      )}
    </div>
  );
}
