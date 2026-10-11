/**
 * /: the store's home page, laid out like Silicon Writings: a short serif title and one paragraph, search, then the
 * catalog (three featured apps, the most installed and the newest as two numbered lists, and the tags), how to get the
 * CLI, what a Silicon can read and call here as a plain spec sheet, the questions people ask, and one quiet line about
 * making an app. Server-rendered; the theme switch and the copy buttons are the only script.
 */
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { ArrowUpRight, Package, Plus } from "lucide-react";
import { AppIndex, FeaturedCard } from "@/components/store/app-card";
import styles from "@/components/store/home.module.css";
import { EmptyState, Notice, SectionHead, StructuredData, styles as store } from "@/components/store/parts";
import { Rich } from "@/components/store/rich";
import { SearchBox } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { CodeBlock } from "@/components/site/code-block";
import { homeSections, listAll, tagCounts } from "@/lib/catalog";
import { FAQ, plainAnswer } from "@/lib/content";
import { formatCount, plural } from "@/lib/format";
import { faqLd, itemListLd, pageMetadata } from "@/lib/seo";
import { CANONICAL_ORIGIN, INSTALL_UNIX, INSTALL_WINDOWS, LINKS, SITE_DESCRIPTION } from "@/lib/site";
import type { App } from "@/lib/types";

const TITLE = "Silicon Apps: apps for Carbons and Silicons";

export const metadata: Metadata = pageMetadata({ title: TITLE, absoluteTitle: true, description: SITE_DESCRIPTION, path: "/" });

const HERO_CODE = `# Find an app
silicon-apps search notes

# Install it, then look around
silicon-apps install briefcase
briefcase --help`;

/** Why the CLI is worth installing, as a short numbered list beside the install commands. */
const CLI_POINTS = [
  { title: "No account needed", text: "Find and install any public app without signing in. You only sign in for private apps and reviews." },
  { title: "Checked before it runs", text: "Every package is checked against its checksum and signature. If an app has nothing for your system, it says so." },
  { title: "Updates on its own", text: "New releases arrive within a minute, on the channel you installed from. Apps never run an updater of their own." },
  { title: "Nine systems", text: "macOS, Linux and Windows on the processors people use. The CLI picks the right package for you." },
];

/** What a Silicon can read and call here: one row per door, the way a spec sheet reads. */
const SILICON_DOORS: Array<{ label: string; body: ReactNode }> = [
  { label: "llms.txt", body: <>The store in plain words at <a href="/llms.txt">/llms.txt</a>, and everything in one file at <a href="/llms-full.txt">/llms-full.txt</a>.</> },
  { label: "The Apps API", body: <>Search, apps, releases and reviews as JSON at <code data-sq-native="">/v1</code>. The whole spec is at <a href="/openapi.json">/openapi.json</a>.</> },
  { label: "Agent card", body: <>What the store can do for an agent, at <a href="/.well-known/agent.json">/.well-known/agent.json</a>.</> },
  { label: "Three commands", body: <>Every app answers <code data-sq-native="">--help</code>, <code data-sq-native="">accounts --json</code> and <code data-sq-native="">login status --json</code> on every system, so you never have to guess.</> },
  { label: "App pages", body: <>Every app page is plain HTML with its install command and SoftwareApplication data, at <code data-sq-native="">/apps/{"{app_id}"}</code>.</> },
  { label: "Robots and sitemap", body: <>Crawlers and agents are named and welcome in <a href="/robots.txt">/robots.txt</a>, and <a href="/sitemap.xml">/sitemap.xml</a> lists every public app and author.</> },
];

export default async function Home() {
  let apps: App[] = [];
  let failed = false;
  try {
    apps = await listAll();
  } catch {
    failed = true;
  }
  const { featured, popular, fresh } = homeSections(apps);
  const tags = tagCounts(apps);
  const installs = apps.reduce((sum, app) => sum + app.installs, 0);

  return (
    <>
      <StructuredData
        graph={[
          { "@type": "WebPage", "@id": `${CANONICAL_ORIGIN}/#webpage`, url: `${CANONICAL_ORIGIN}/`, name: TITLE, description: SITE_DESCRIPTION, isPartOf: { "@id": `${CANONICAL_ORIGIN}/#website` } },
          ...(apps.length ? [itemListLd("Apps on Silicon Apps", apps.filter(app => app.visibility === "public").slice(0, 30))] : []),
          faqLd(FAQ.map(faq => ({ question: faq.question, answer: plainAnswer(faq) }))),
        ]}
      />

      {/* Hero ------------------------------------------------------------------------------------------------------ */}
      <section className={styles.hero} aria-labelledby="hero-title">
        <div className={styles.heroInner}>
          <div className={styles.heroCopy}>
            <p className={styles.kicker}>
              <span>Silicon Apps</span>
              {apps.length ? (
                <>
                  <span aria-hidden="true">·</span>
                  <span>{plural(apps.length, "app")}</span>
                  {installs ? (
                    <>
                      <span aria-hidden="true">·</span>
                      <span>{formatCount(installs)} {installs === 1 ? "install" : "installs"}</span>
                    </>
                  ) : null}
                </>
              ) : null}
            </p>
            <h1 id="hero-title" className={styles.heroTitle}>Apps for Carbons and Silicons.</h1>
            <p className={styles.heroLede}>
              Every app here is a command line app first, so a Silicon can install it, run it and find its way around it on
              its own. One command installs it, and it keeps itself up to date.
            </p>
            <div className={styles.heroSearch}>
              <SearchBox large id="hero-q" />
            </div>
            {tags.length ? (
              <div className={styles.heroTags}>
                <span className={styles.heroTagsLabel}>Try</span>
                <ul className={styles.inlineTags} role="list" aria-label="Popular tags">
                  {tags.slice(0, 5).map(({ tag }) => (
                    <li key={tag}><a href={`/search?tag=${encodeURIComponent(tag)}`}>{tag}</a></li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
          <div className={styles.heroArt}>
            <CodeBlock code={HERO_CODE} lang="sh" title="In your terminal" />
            <p className={styles.heroNote}>No account needed to find or install a public app. <a href="#get-the-cli">Get the CLI</a></p>
          </div>
        </div>
      </section>

      {/* The catalog ----------------------------------------------------------------------------------------------- */}
      <div className={styles.catalog}>
        <div className={styles.inner}>
          {failed ? (
            <Notice tone="danger" title="The catalog is not answering right now">
              <p>We could not reach the Apps service. Try again in a moment, or run <code>silicon-apps search</code> in your terminal.</p>
            </Notice>
          ) : null}

          {!failed && apps.length === 0 ? (
            <EmptyState icon={<Package size={18} strokeWidth={1.75} />} title="No apps here yet" actions={<Action href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make the first one<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>}>
              <p>Apps show up here the moment their authors publish them.</p>
            </EmptyState>
          ) : null}

          {featured.length ? (
            <section aria-labelledby="featured-title">
              <SectionHead id="featured-title" title="Featured" more={{ href: "/search", label: "Every app" }}>Loved by the Carbons and Silicons who use them.</SectionHead>
              <ul className={store.featuredGrid} role="list">
                {featured.map(app => (
                  <li key={app.app_id}><FeaturedCard app={app} /></li>
                ))}
              </ul>
            </section>
          ) : null}

          {popular.length || fresh.length ? (
            <div className={styles.lists}>
              {popular.length ? (
                <section aria-labelledby="popular-title" className={styles.list}>
                  <SectionHead id="popular-title" title="Most installed" />
                  <AppIndex apps={popular} numbered trailing="installs" />
                </section>
              ) : null}
              {fresh.length ? (
                <section aria-labelledby="new-title" className={styles.list}>
                  <SectionHead id="new-title" title="Just arrived" />
                  <AppIndex apps={fresh} trailing="date" />
                </section>
              ) : null}
            </div>
          ) : null}

          {tags.length ? (
            <section aria-labelledby="tags-title">
              <SectionHead id="tags-title" title="Browse by tag" />
              <ul className={store.chips} role="list">
                {tags.slice(0, 40).map(({ tag, count }) => (
                  <li key={tag}>
                    <a className={store.chip} data-sq="surface" href={`/search?tag=${encodeURIComponent(tag)}`}>
                      {tag}<span className={store.chipCount}>{count}<span className="sr-only"> {count === 1 ? "app" : "apps"}</span></span>
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </div>
      </div>

      {/* Get the CLI ----------------------------------------------------------------------------------------------- */}
      <section className={styles.section} id="get-the-cli" aria-labelledby="cli-title">
        <div className={`${styles.inner} ${styles.split}`}>
          <div className={styles.sectionHead}>
            <p className={styles.kicker}>Get the CLI</p>
            <h2 id="cli-title" className={styles.sectionTitle}>One install, then every app is a command away.</h2>
            <p className={styles.sectionLede}>
              The installer picks the right download for your system, checks it and installs the latest{" "}
              <code data-sq-native="">silicon-apps</code>. It keeps itself, and every app you install, up to date.
            </p>
            <ol className={styles.points} role="list">
              {CLI_POINTS.map((point, index) => (
                <li key={point.title}>
                  <span className={styles.pointNumber} aria-hidden="true">{String(index + 1).padStart(2, "0")}</span>
                  <div>
                    <h3 className={styles.pointTitle}>{point.title}</h3>
                    <p className={styles.pointText}>{point.text}</p>
                  </div>
                </li>
              ))}
            </ol>
          </div>
          <div className={styles.cliCode}>
            <CodeBlock code={INSTALL_UNIX} lang="sh" title="macOS and Linux" />
            <CodeBlock code={INSTALL_WINDOWS} lang="powershell" title="Windows PowerShell" />
            <p className={styles.miniText}>Want Silicon Accounts at the same time? Follow the <a href={LINKS.installDocs} target="_blank" rel="noopener noreferrer">install guide</a>.</p>
          </div>
        </div>
      </section>

      {/* For Silicons ---------------------------------------------------------------------------------------------- */}
      <section className={styles.section} id="for-silicons" aria-labelledby="silicons-title">
        <div className={`${styles.inner} ${styles.split}`}>
          <div className={styles.sectionHead}>
            <p className={styles.kicker}>For Silicons</p>
            <h2 id="silicons-title" className={styles.sectionTitle}>Everything here is plain HTML and an API.</h2>
            <p className={styles.sectionLede}>
              You can find and install apps without a browser. Every app was made with Silicons in mind: it installs with one
              command, updates itself and answers the same three commands.
            </p>
          </div>
          <dl className={styles.spec}>
            {SILICON_DOORS.map(door => (
              <div key={door.label}>
                <dt>{door.label}</dt>
                <dd>{door.body}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      {/* Questions ------------------------------------------------------------------------------------------------- */}
      <section className={styles.section} id="faq" aria-labelledby="faq-title">
        <div className={`${styles.inner} ${styles.split}`}>
          <div className={styles.sectionHead}>
            <p className={styles.kicker}>Questions</p>
            <h2 id="faq-title" className={styles.sectionTitle}>Questions, answered.</h2>
          </div>
          <div className={styles.faqList}>
            {FAQ.map(faq => (
              <details key={faq.id} className={styles.faqItem} id={faq.id}>
                <summary className={styles.faqQuestion}>
                  <h3 className={styles.faqHeading}>{faq.question}</h3>
                  <Plus size={18} strokeWidth={1.75} aria-hidden="true" className={styles.faqIcon} />
                </summary>
                <p className={styles.faqAnswer}><Rich text={faq.answer} /></p>
              </details>
            ))}
          </div>
        </div>
      </section>

      {/* Make an app: one quiet line pointing at the developer site ------------------------------------------------- */}
      <section className={styles.pointerWrap} aria-labelledby="build-title">
        <div className={styles.pointer}>
          <div className={styles.pointerText}>
            <h2 id="build-title" className={styles.pointerTitle}>Made something useful?</h2>
            <p>Making an app, publishing it here and adding sign-in all live on Silicon Developer.</p>
          </div>
          <div className={styles.pointerActions}>
            <Action href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make an app<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>
            <Action href={LINKS.accountsDocs} target="_blank" rel="noopener noreferrer" variant="secondary">Add sign-in<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>
          </div>
        </div>
      </section>
    </>
  );
}
