/**
 * /playground/whats-new: the app page's What's new as it is, beside four other designs (Anil, 2026-11-18, the account
 * site's lab way: "go through similar works, and come up with 4 very different designs"), on a real app's release
 * history from the live catalog. Under `next dev` only (a 404 in production builds, never indexed).
 *
 *   ?v=   now (the page's, Store, picked 2026-11-18: components/app/whats-new.tsx), b (Timeline), c (Notes window),
 *         d (Pulse); the other designs are in components/app/whats-new-lab/designs.tsx
 *   ?app= the app (Briefcase by default: 7 releases; Silicon has the longest notes, Ring a newer development release)
 * Plain links, so the page works without script, as the store does.
 */
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { notFound } from "next/navigation";
import { WhatsNew } from "@/components/app/whats-new";
import { PulseDesign, TimelineDesign, WindowDesign } from "@/components/app/whats-new-lab/designs";
import { Page } from "@/components/parts/page";
import { apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { releaseHistory } from "@/lib/releases";
import type { App, Release } from "@/lib/types";
import styles from "../app-header/lab.module.css";

export const metadata: Metadata = { title: "What's new lab", robots: { index: false, follow: false } };

type Render = (app: App, history: Release[]) => ReactNode;
const DESIGNS: { id: string; label: string; line: string; render: Render }[] = [
  { id: "now", label: "Now · Store", line: "The page's, after the App Store (picked 2026-11-18): the version and how long ago, the notes cut to three lines with “more”, then folds for version history, withdrawn releases and other ways to install.", render: (app, history) => <WhatsNew app={app} history={history} /> },
  { id: "b", label: "B · Timeline", line: "After GitHub Releases and Linear's changelog: a dated rail, newest first, the three newest shown and the rest behind Show all.", render: (app, history) => <TimelineDesign app={app} history={history} /> },
  { id: "c", label: "C · Notes window", line: "After Sparkle's update window on the Mac: the versions down the left, the chosen one's notes on the right.", render: (app, history) => <WindowDesign app={app} history={history} /> },
  { id: "d", label: "D · Pulse", line: "After the Chrome Web Store and npm: how lately and how often first, a dot per release in time, then the latest notes.", render: (app, history) => <PulseDesign app={app} history={history} /> },
];

const APPS = [
  { id: "briefcase", label: "Briefcase (7)" },
  { id: "silicon", label: "Silicon (long notes)" },
  { id: "ring", label: "Ring (newer dev)" },
  { id: "dm", label: "DM (short notes)" },
];

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

export default async function WhatsNewLab({ searchParams }: Props) {
  if (process.env.NODE_ENV === "production") notFound();
  const params = await searchParams;
  const design = DESIGNS.find(item => item.id === params.v) ?? DESIGNS[0];
  const appId = typeof params.app === "string" && APP_ID_PATTERN.test(params.app) ? params.app : "briefcase";
  const app = await apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`).catch(() => null);
  const history = app ? await releaseHistory(app) : [];
  const href = (next: { v?: string; app?: string }) => `/playground/whats-new?${new URLSearchParams({ v: next.v ?? design.id, app: next.app ?? appId })}`;

  return (
    <Page wide>
      <div className={styles.lab}>
        <h1 className="sr-only">What&apos;s new lab</h1>
        <div className={styles.controls}>
          <nav className={styles.tabs} aria-label="Designs">
            {DESIGNS.map(item => <a key={item.id} href={href({ v: item.id })} className={styles.tab} aria-current={item.id === design.id ? "page" : undefined}>{item.label}</a>)}
          </nav>
          <nav className={styles.apps} aria-label="Apps">
            {APPS.map(item => <a key={item.id} href={href({ app: item.id })} className={styles.app} aria-current={item.id === appId ? "page" : undefined}>{item.label}</a>)}
          </nav>
        </div>
        <p className={styles.line}>{design.line}</p>
        <hr className={styles.rule} />
        {app ? design.render(app, history) : <p className={styles.line}>The catalog has no app “{appId}” (or isn’t answering).</p>}
      </div>
    </Page>
  );
}
