/**
 * /playground/app-header: the app page's header as it is (Editorial) beside Cover, the two kept of four designs tried
 * (Anil, 2026-11-18, the account site's lab way), on a real app
 * from the live catalog. Under `next dev` only (a 404 in production builds, never indexed). Nothing here acts beyond
 * what the header itself links to.
 *
 *   ?v=   now (the page's own, Editorial, components/app/app-header.tsx) or b (Cover, kept to compare,
 *         components/app/header-lab/headers.tsx); Spotlight and Install card were tried and dropped (in git)
 *   ?dev= how Now shows the developer: chips (the page's), profile, faces or plain (Developers in parts.tsx)
 *   ?app= the app to show (Ring by default: it has a picture of its own; Silicon Browser has none, so B shows its
 *         gradient)
 * Under the header, the page's screenshots (the app's own, else stand-ins, since no app has any yet: the store's
 * pictures in three desktop and two mobile frames), About, What's new and Ratings and reviews (eight made-up stand-ins, as no app has a review yet), as the page has them. The tabs and the apps are plain links, so the page works without script, as the store does.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { About } from "@/components/app/about";
import { AppHeader } from "@/components/app/app-header";
import { Screenshots } from "@/components/app/screenshots";
import { Reviews } from "@/components/app/reviews";
import { WhatsNew } from "@/components/app/whats-new";
import { CoverHeader } from "@/components/app/header-lab/headers";
import { DEVELOPER_STYLES, type DeveloperStyle } from "@/components/app/header-lab/parts";
import { Page } from "@/components/parts/page";
import { apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { SAMPLE_LIST, SAMPLE_SHOTS } from "@/components/app/samples";
import type { Account, App } from "@/lib/types";
import styles from "./lab.module.css";

export const metadata: Metadata = { title: "Header lab", robots: { index: false, follow: false } };

const DESIGNS = [
  { id: "now", label: "Now · Editorial", line: "The page's header: the icon with the name and the lede beside it, then a row of labelled facts with the actions at its end." },
  { id: "b", label: "B · Cover", line: "Kept to compare: the app's picture across the top (else a soft gradient), the icon rising out of its corner, the actions on the right of the name's row." },
] as const;

/** Who reads the reviews (?as=): signed out, signed in without a review (the form to write one), or signed in as the
 * second sample's writer (their review first, marked You, and the form holding it). Previews only: the form doesn't post. */
const VISITORS = [
  { id: "out", label: "Signed out" },
  { id: "new", label: "Signed in" },
  { id: "mine", label: "Has a review" },
] as const;
const SAMPLE_VISITOR: Account = { uuid: "sample-visitor", id: "c:sample-visitor", display_name: "Sample visitor" };
const SAMPLE_REVIEWER: Account = { uuid: "s2", id: "si:sample-scout", display_name: "Sample scout" };

const DEVELOPER_LABELS: Record<DeveloperStyle, string> = { chips: "Chips", profile: "Profile", faces: "Faces", plain: "Plain" };


const APPS = [
  { id: "ring", label: "Ring" },
  { id: "dm", label: "DM" },
  { id: "briefcase", label: "Briefcase" },
  { id: "browser", label: "Browser (no picture)" },
];

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

export default async function HeaderLab({ searchParams }: Props) {
  if (process.env.NODE_ENV === "production") notFound();
  const params = await searchParams;
  const v = typeof params.v === "string" ? params.v : "now";
  const design = DESIGNS.find(item => item.id === v) ?? DESIGNS[0];
  const appId = typeof params.app === "string" && APP_ID_PATTERN.test(params.app) ? params.app : "ring";
  const app = await apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`).catch(() => null);
  const dev = DEVELOPER_STYLES.find(item => item === params.dev) ?? "chips";
  const as = VISITORS.find(item => item.id === params.as)?.id ?? "out";
  const href = (next: { v?: string; app?: string; dev?: string; as?: string }) => `/playground/app-header?${new URLSearchParams({ v: next.v ?? design.id, app: next.app ?? appId, dev: next.dev ?? dev, as: next.as ?? as })}`;

  return (
    <Page wide>
      <div className={styles.lab}>
        <div className={styles.controls}>
          <nav className={styles.tabs} aria-label="Designs">
            {DESIGNS.map(item => (
              <a key={item.id} href={href({ v: item.id })} className={styles.tab} aria-current={item.id === design.id ? "page" : undefined}>{item.label}</a>
            ))}
          </nav>
          <nav className={styles.apps} aria-label="Apps">
            {APPS.map(item => (
              <a key={item.id} href={href({ app: item.id })} className={styles.app} aria-current={item.id === appId ? "page" : undefined}>{item.label}</a>
            ))}
          </nav>
        </div>
        <p className={styles.line}>{design.line}</p>
        {design.id === "now" ? (
          <nav className={styles.apps} aria-label="How the developer shows">
            <span className={styles.line}>Developer:</span>
            {DEVELOPER_STYLES.map(item => (
              <a key={item} href={href({ dev: item })} className={styles.app} aria-current={item === dev ? "page" : undefined}>{DEVELOPER_LABELS[item]}</a>
            ))}
          </nav>
        ) : null}
        <nav className={styles.apps} aria-label="Who is looking">
          <span className={styles.line}>Reviews as:</span>
          {VISITORS.map(item => (
            <a key={item.id} href={href({ as: item.id })} className={styles.app} aria-current={item.id === as ? "page" : undefined}>{item.label}</a>
          ))}
        </nav>
        <hr className={styles.rule} />
        {app ? (
          <div className={styles.page}>
            {design.id === "b" ? <CoverHeader app={app} /> : <AppHeader app={app} developers={dev} />}
            <Screenshots app={app} items={app.carousel?.length ? app.carousel : SAMPLE_SHOTS} />
            <About app={app} />
            <WhatsNew app={app} />
            <Reviews app={app} reviews={SAMPLE_LIST} account={as === "out" ? null : as === "mine" ? SAMPLE_REVIEWER : SAMPLE_VISITOR} preview />
          </div>
        ) : <p className={styles.line}>The catalog has no app “{appId}” (or isn’t answering).</p>}
      </div>
    </Page>
  );
}
