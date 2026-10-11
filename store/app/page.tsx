/**
 * /: the store's home page, kept as quiet as Silicon Writings: one column with a serif title, one paragraph and the
 * search, the apps as one list, then how to get the CLI, what a Silicon can read here, and the questions people ask.
 * Server-rendered; the copy buttons are the only script.
 */
import type { Metadata } from "next";
import { ArrowRight, Package, Plus } from "lucide-react";
import { AppList } from "@/components/store/app-card";
import styles from "@/components/store/home.module.css";
import { EmptyState, Notice, StructuredData } from "@/components/store/parts";
import { Rich } from "@/components/store/rich";
import { SearchBox } from "@/components/store/search-form";
import { Action } from "@/components/site/action";
import { CodeBlock } from "@/components/site/code-block";
import { listAll } from "@/lib/catalog";
import { FAQ, plainAnswer } from "@/lib/content";
import { faqLd, itemListLd, pageMetadata } from "@/lib/seo";
import { CANONICAL_ORIGIN, INSTALL_UNIX, INSTALL_WINDOWS, LINKS, SITE_DESCRIPTION } from "@/lib/site";
import type { App } from "@/lib/types";

const TITLE = "Silicon Apps: apps for Carbons and Silicons";
const SHOWN = 10;

export const metadata: Metadata = pageMetadata({ title: TITLE, absoluteTitle: true, description: SITE_DESCRIPTION, path: "/" });

/** Best first: rated, then installed, then the ones with a logo; ties by name. */
function ranked(apps: App[]): App[] {
  const score = (app: App) => (app.rating ?? 0) * Math.min(app.review_count, 5) + Math.log10(app.installs + 1) * 4 + (app.logo ? 0.5 : 0);
  return [...apps].sort((a, b) => score(b) - score(a) || a.name.localeCompare(b.name));
}

export default async function Home() {
  let apps: App[] = [];
  let failed = false;
  try {
    apps = await listAll();
  } catch {
    failed = true;
  }
  const shown = ranked(apps).slice(0, SHOWN);

  return (
    <div className={styles.page}>
      <StructuredData
        graph={[
          { "@type": "WebPage", "@id": `${CANONICAL_ORIGIN}/#webpage`, url: `${CANONICAL_ORIGIN}/`, name: TITLE, description: SITE_DESCRIPTION, isPartOf: { "@id": `${CANONICAL_ORIGIN}/#website` } },
          ...(apps.length ? [itemListLd("Apps on Silicon Apps", apps.filter(app => app.visibility === "public").slice(0, 30))] : []),
          faqLd(FAQ.map(faq => ({ question: faq.question, answer: plainAnswer(faq) }))),
        ]}
      />

      <header className={styles.hero}>
        <p className={styles.kicker}>Silicon Apps</p>
        <h1 className={styles.title}>Apps for Carbons and Silicons.</h1>
        <p className={styles.lede}>
          Every app here is a command line app first, so a Silicon can install it and find its way around on its own. One
          command installs it, and it keeps itself up to date.
        </p>
        <SearchBox id="hero-q" />
      </header>

      <section className={styles.section} aria-labelledby="apps-title">
        <div className={styles.head}>
          <h2 id="apps-title" className={styles.heading}>Apps</h2>
        </div>
        {failed ? (
          <Notice tone="danger" title="The catalog is not answering right now">
            <p>We could not reach the Apps service. Try again in a moment, or run <code>silicon-apps search</code> in your terminal.</p>
          </Notice>
        ) : apps.length === 0 ? (
          <EmptyState icon={<Package size={18} strokeWidth={1.75} />} title="No apps here yet" actions={<Action href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make the first one</Action>}>
            <p>Apps show up here the moment their authors publish them.</p>
          </EmptyState>
        ) : (
          <>
            <AppList apps={shown} label="Apps" />
            <div>
              <Action href="/search" variant="secondary">View all {apps.length} apps<ArrowRight size={15} strokeWidth={1.75} aria-hidden="true" /></Action>
            </div>
          </>
        )}
      </section>

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

      <section className={styles.section} id="faq" aria-labelledby="faq-title">
        <div className={styles.head}>
          <h2 id="faq-title" className={styles.heading}>Questions, answered</h2>
        </div>
        <div className={styles.faq}>
          {FAQ.map(faq => (
            <details key={faq.id} className={styles.faqItem} id={faq.id}>
              <summary className={styles.faqQuestion}>
                <h3 className={styles.faqHeading}>{faq.question}</h3>
                <Plus size={16} strokeWidth={1.75} aria-hidden="true" className={styles.faqIcon} />
              </summary>
              <p className={styles.faqAnswer}><Rich text={faq.answer} /></p>
            </details>
          ))}
        </div>
      </section>

      <p className={styles.closing}>
        Made something useful? <a href={LINKS.appsDocs} target="_blank" rel="noopener noreferrer">Make an app</a> or{" "}
        <a href={LINKS.accountsDocs} target="_blank" rel="noopener noreferrer">add sign-in</a> on Silicon Developer.
      </p>
    </div>
  );
}
