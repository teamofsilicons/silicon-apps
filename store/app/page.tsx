/**
 * /: the store's home page. What Silicon Apps is in one line, search, the catalog (featured, popular, new and tags),
 * how to get the CLI, what Silicons can read and call here, one short pointer to the developer site for making apps,
 * and the questions people ask. Server-rendered; the theme switch and the copy buttons are the only script.
 */
import type { Metadata } from "next";
import { ArrowUpRight, Bot, Braces, FileText, Globe, KeyRound, ListTree, Package, Plus, RefreshCw, ShieldCheck, Terminal } from "lucide-react";
import { AppGrid, FeaturedCard } from "@/components/store/app-card";
import styles from "@/components/store/home.module.css";
import { EmptyState, Notice, SectionHead, StructuredData, styles as store } from "@/components/store/parts";
import { Rich } from "@/components/store/rich";
import { SearchBox } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { CodeBlock } from "@/components/site/code-block";
import { homeSections, listAll, tagCounts } from "@/lib/catalog";
import { FAQ, plainAnswer } from "@/lib/content";
import { faqLd, itemListLd, pageMetadata } from "@/lib/seo";
import { CANONICAL_ORIGIN, INSTALL_UNIX, INSTALL_WINDOWS, LINKS, SITE_DESCRIPTION } from "@/lib/site";
import type { App } from "@/lib/types";

const TITLE = "Silicon Apps: apps for Carbons and Silicons";

export const metadata: Metadata = pageMetadata({ title: TITLE, absoluteTitle: true, description: SITE_DESCRIPTION, path: "/" });

const HERO_CODE = `# Find an app (spelling mistakes are fine)
silicon-apps search notes

# Install it, then find your way around
silicon-apps install briefcase
briefcase --help

# Sign in for private apps and reviews
silicon-accounts login --app silicon-apps
silicon-apps login --slt TOKEN`;

const ICON = { size: 18, strokeWidth: 1.75 } as const;

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
        <div className={`${styles.heroInner} ${styles.storeHero}`}>
          <div className={styles.heroCopy}>
            <p className={styles.heroBadge} data-sq="surface"><span className={styles.dot} data-sq-native="" aria-hidden="true" />For Carbons and Silicons</p>
            <h1 id="hero-title" className={styles.heroTitle}>Apps for Carbons and Silicons</h1>
            <p className={styles.heroLede}>
              Silicon Apps is the store of the Silicon ecosystem. Every app here is made for Silicons and Carbons alike, is a
              command line app first, installs with one command and keeps itself up to date.
            </p>
            <div className={styles.heroSearch}>
              <SearchBox large id="hero-q" />
            </div>
            {tags.length ? (
              <div className={styles.heroTags}>
                <span className={styles.heroTagsLabel}>Popular tags</span>
                <ul className={store.chips} role="list" aria-label="Popular tags">
                  {tags.slice(0, 5).map(({ tag }) => (
                    <li key={tag}><a className={store.chip} data-sq="surface" href={`/search?tag=${encodeURIComponent(tag)}`}>{tag}</a></li>
                  ))}
                </ul>
              </div>
            ) : null}
            <p className={styles.heroNote}>No account needed to find or install a public app.</p>
          </div>
          <div className={styles.heroArt}>
            <CodeBlock code={HERO_CODE} lang="sh" title="As a Silicon" />
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
            <EmptyState icon={<Package {...ICON} />} title="No apps here yet" actions={<Action href={LINKS.appsDocs}>Make the first one<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>}>
              <p>Apps show up here the moment their authors publish them.</p>
            </EmptyState>
          ) : null}

          {featured.length ? (
            <section aria-labelledby="featured-title">
              <SectionHead id="featured-title" title="Featured" more={{ href: "/search", label: "All apps" }}>Loved by the Carbons and Silicons who use them.</SectionHead>
              <ul className={store.featuredGrid} role="list">
                {featured.map(app => (
                  <li key={app.app_id}><FeaturedCard app={app} /></li>
                ))}
              </ul>
            </section>
          ) : null}

          {popular.length ? (
            <section aria-labelledby="popular-title">
              <SectionHead id="popular-title" title="Popular">Installed the most.</SectionHead>
              <AppGrid apps={popular} />
            </section>
          ) : null}

          {fresh.length ? (
            <section aria-labelledby="new-title">
              <SectionHead id="new-title" title="New">Just arrived in the store.</SectionHead>
              <AppGrid apps={fresh} />
            </section>
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
      <section className={`${styles.section} ${styles.band} ${styles.cliBand}`} id="get-the-cli" aria-labelledby="cli-title">
        <div className={styles.inner}>
          <div className={styles.sectionHead}>
            <p className={styles.eyebrow}>Get the CLI</p>
            <h2 id="cli-title" className={styles.sectionTitle}>Install silicon-apps once, and every app is one command away</h2>
            <p className={styles.sectionLede}>
              The installer finds the right download for your OS and processor, checks its checksum and installs the latest{" "}
              <code data-sq-native="">silicon-apps</code>. It keeps itself and every app you install up to date.
            </p>
          </div>
          <ul className={`${styles.features} ${styles.featuresCompact}`} role="list">
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><KeyRound {...ICON} /></span>
              <h3 className={styles.featureTitle}>No account needed</h3>
              <p className={styles.featureText}>Find and install any public app without signing in. Sign in only for private apps and reviews.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><ShieldCheck {...ICON} /></span>
              <h3 className={styles.featureTitle}>Checked before it runs</h3>
              <p className={styles.featureText}>Every package is checked against its checksum, and an app with no package for your system says so.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><RefreshCw {...ICON} /></span>
              <h3 className={styles.featureTitle}>Updates on their own</h3>
              <p className={styles.featureText}>New releases arrive within a minute, on the channel you installed from. Apps never update themselves.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><Terminal {...ICON} /></span>
              <h3 className={styles.featureTitle}>Nine systems</h3>
              <p className={styles.featureText}>macOS, Linux and Windows on the processors people use. The CLI picks the right package for you.</p>
            </li>
          </ul>
          <div className={styles.cliCode}>
            <CodeBlock code={INSTALL_UNIX} lang="sh" title="macOS and Linux" />
            <CodeBlock code={INSTALL_WINDOWS} lang="powershell" title="Windows PowerShell" />
            <p className={styles.miniText}>Want Silicon Accounts at the same time? Follow the <a href={LINKS.installDocs}>install guide</a>.</p>
          </div>
        </div>
      </section>

      {/* For Silicons ---------------------------------------------------------------------------------------------- */}
      <section className={styles.section} id="for-silicons" aria-labelledby="silicons-title">
        <div className={styles.inner}>
          <div className={styles.sectionHead}>
            <p className={styles.eyebrow}>For Silicons</p>
            <h2 id="silicons-title" className={styles.sectionTitle}>Everything here is plain HTML and an API</h2>
            <p className={styles.sectionLede}>
              Every app in this store was made with Silicons in mind: it installs with one command, it updates itself, and it
              answers the same three commands, so you never have to guess. You can find and install apps without a browser.
            </p>
          </div>
          <ul className={styles.features} role="list">
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><FileText {...ICON} /></span>
              <h3 className={styles.featureTitle}>llms.txt</h3>
              <p className={styles.featureText}>The store in plain words: <a href="/llms.txt">/llms.txt</a>, and everything in one file at <a href="/llms-full.txt">/llms-full.txt</a>.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><Braces {...ICON} /></span>
              <h3 className={styles.featureTitle}>The Apps API</h3>
              <p className={styles.featureText}>Search, apps, releases and reviews as JSON at <code data-sq-native="">/v1</code>. The full spec is at <a href="/openapi.json">/openapi.json</a>.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><Bot {...ICON} /></span>
              <h3 className={styles.featureTitle}>Agent card</h3>
              <p className={styles.featureText}>What the store can do for an agent, at <a href="/.well-known/agent.json">/.well-known/agent.json</a>.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><Terminal {...ICON} /></span>
              <h3 className={styles.featureTitle}>The three commands</h3>
              <p className={styles.featureText}>Every app answers <code data-sq-native="">--help</code>, <code data-sq-native="">accounts --json</code> and <code data-sq-native="">login status --json</code> on every system.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><Globe {...ICON} /></span>
              <h3 className={styles.featureTitle}>Pages that read like data</h3>
              <p className={styles.featureText}>Every app page is server-rendered with its install command and SoftwareApplication data, at <code data-sq-native="">/apps/{"{app_id}"}</code>.</p>
            </li>
            <li className={styles.feature}>
              <span className={styles.featureIcon} data-sq="surface" aria-hidden="true"><ListTree {...ICON} /></span>
              <h3 className={styles.featureTitle}>Robots and sitemap</h3>
              <p className={styles.featureText}>Crawlers and agents are named and welcome in <a href="/robots.txt">/robots.txt</a>, and <a href="/sitemap.xml">/sitemap.xml</a> lists every public app and author.</p>
            </li>
          </ul>
        </div>
      </section>

      {/* Questions ------------------------------------------------------------------------------------------------- */}
      <section className={`${styles.section} ${styles.band}`} id="faq" aria-labelledby="faq-title">
        <div className={`${styles.inner} ${styles.faq}`}>
          <div className={styles.sectionHead}>
            <p className={styles.eyebrow}>Questions</p>
            <h2 id="faq-title" className={styles.sectionTitle}>Questions, answered</h2>
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

      {/* Make an app: one short pointer to the developer site -------------------------------------------------------- */}
      <section className={styles.pointerWrap} aria-labelledby="build-title">
        <div className={styles.pointer} data-sq="surface">
          <div className={styles.pointerText}>
            <h2 id="build-title" className={styles.pointerTitle}>Made something useful?</h2>
            <p>Making an app, publishing it here and adding sign-in all live on Silicon Developer.</p>
          </div>
          <div className={styles.pointerActions}>
            <Action href={LINKS.appsDocs}>Make an app<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>
            <Action href={LINKS.accountsDocs} variant="secondary">Add sign-in<ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" /></Action>
          </div>
        </div>
      </section>
    </>
  );
}
