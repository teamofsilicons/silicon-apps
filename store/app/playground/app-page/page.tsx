/**
 * /playground/app-page: the whole app page in four layouts (Anil, 2026-11-18: "a bit of asymmetry in the information
 * flow": the header and Developer run the page's full width, About, What's new, Compatibility and the reviews' summary
 * a reading width; "ideally a uniform would be cool"), on a real app from the live catalog. Under `next dev` only (a 404
 * in production builds, never indexed). The same components as the page; only where they sit changes.
 *
 *   ?layout= before   the page before Sidebar was picked
 *            sidebar  (the default) the page's (picked 2026-11-18, components/app/app-layout.tsx): a reading column (About, What's new, Compatibility, reviews) and a sidebar beside it (the install
 *                     command, the facts, the developer card), kept in view as you scroll; More by across the foot.
 *                     After npm, GitHub, crates.io and the VS Code Marketplace
 *            labels   every section on one grid: its title in a label column on the left, what it holds on the right.
 *                     After Linear's changelog and Stripe's docs
 *            column   everything at the reading width (70ch), the header and Developer included; one left edge, one
 *                     right edge. After Raycast's store and a long read
 *   ?app=    the app (Ring by default)
 * Screenshots and reviews are the playgrounds' stand-ins (components/app/samples.ts) where the app has none; the review
 * form is a preview that never posts. Plain links, so the page works without script.
 */
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { notFound } from "next/navigation";
import { About } from "@/components/app/about";
import { AppHeader } from "@/components/app/app-header";
import { SidebarLayout } from "@/components/app/app-layout";
import { Compatibility } from "@/components/app/compatibility";
import { Developer } from "@/components/app/developer";
import { Reviews } from "@/components/app/reviews";
import { SAMPLE_LIST, SAMPLE_SHOTS } from "@/components/app/samples";
import { Screenshots } from "@/components/app/screenshots";
import { WhatsNew } from "@/components/app/whats-new";
import { Page } from "@/components/parts/page";
import { apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { developerWork, type DeveloperWork } from "@/lib/developers";
import { releaseHistory } from "@/lib/releases";
import type { App, Release } from "@/lib/types";
import lab from "../app-header/lab.module.css";
import styles from "./layouts.module.css";

export const metadata: Metadata = { title: "App page lab", robots: { index: false, follow: false } };

type Data = { app: App; history: Release[]; work: DeveloperWork };

/** The sections under the header, as the page has them; `developer` false leaves Developer out (the sidebar has it). */
function Body({ app, history, work, developer = true }: Data & { developer?: boolean }) {
  return (
    <>
      <Screenshots app={app} items={app.carousel?.length ? app.carousel : SAMPLE_SHOTS} />
      <About app={app} />
      <WhatsNew app={app} history={history} />
      <Compatibility app={app} />
      {developer ? <Developer app={app} work={work} /> : null}
      <Reviews app={app} reviews={SAMPLE_LIST} account={null} preview />
    </>
  );
}

const LAYOUTS: { id: string; label: string; line: string; render: (data: Data) => ReactNode }[] = [
  {
    id: "before",
    label: "Before",
    line: "The page before Sidebar was picked: the header and Developer across the page, About, What’s new, Compatibility and the reviews’ summary at a reading width.",
    render: data => <><AppHeader app={data.app} /><Body {...data} /></>,
  },
  {
    id: "sidebar",
    label: "Now · Sidebar",
    line: "The page’s (picked 2026-11-18), after npm, GitHub and the VS Code Marketplace: reading on the left, getting it and who made it on the right (kept in view as you scroll), More by across the foot. On a phone the sidebar comes first, under the name.",
    render: ({ app, history, work }) => <SidebarLayout app={app} work={work}><Body app={app} history={history} work={work} developer={false} /></SidebarLayout>,
  },
  {
    id: "labels",
    label: "3 · Label column",
    line: "After Linear’s changelog and Stripe’s docs: every section on one grid, its title on the left (in line with the header’s icon), what it holds on the right. On a phone the titles go back above.",
    render: data => <div className={styles.labels}><AppHeader app={data.app} /><Body {...data} /></div>,
  },
  {
    id: "column",
    label: "2 · One column",
    line: "Everything at the reading width (70ch), the header and Developer included: one left edge and one right edge all the way down.",
    render: data => <div className={styles.column}><AppHeader app={data.app} /><Body {...data} /></div>,
  },
];

const APPS = [
  { id: "ring", label: "Ring" },
  { id: "briefcase", label: "Briefcase (two developers)" },
  { id: "silicon", label: "Silicon (long notes)" },
  { id: "omni", label: "Omni (Linux only)" },
];

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

export default async function AppPageLab({ searchParams }: Props) {
  if (process.env.NODE_ENV === "production") notFound();
  const params = await searchParams;
  const layout = LAYOUTS.find(item => item.id === params.layout) ?? LAYOUTS[1];
  const appId = typeof params.app === "string" && APP_ID_PATTERN.test(params.app) ? params.app : "ring";
  const app = await apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`).catch(() => null);
  const [history, work] = app ? await Promise.all([releaseHistory(app), developerWork(app)]) : [[], { totals: {}, more: [] }];
  const href = (next: { layout?: string; app?: string }) => `/playground/app-page?${new URLSearchParams({ layout: next.layout ?? layout.id, app: next.app ?? appId })}`;

  return (
    <Page wide>
      <div className={lab.lab}>
        <div className={lab.controls}>
          <nav className={lab.tabs} aria-label="Layouts">
            {LAYOUTS.map(item => <a key={item.id} href={href({ layout: item.id })} className={lab.tab} aria-current={item.id === layout.id ? "page" : undefined}>{item.label}</a>)}
          </nav>
          <nav className={lab.apps} aria-label="Apps">
            {APPS.map(item => <a key={item.id} href={href({ app: item.id })} className={lab.app} aria-current={item.id === appId ? "page" : undefined}>{item.label}</a>)}
          </nav>
        </div>
        <p className={lab.line}>{layout.line}</p>
        <hr className={lab.rule} />
        {app ? <article aria-labelledby="app-name">{layout.render({ app, history, work })}</article> : <p className={lab.line}>The catalog has no app “{appId}” (or isn’t answering).</p>}
      </div>
    </Page>
  );
}
