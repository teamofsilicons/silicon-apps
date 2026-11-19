/**
 * /playground/developer: the app page's Developer as it is, beside three other designs (Anil, 2026-11-18, the account
 * site's lab way: "what other design can we think of here?", More by included), on a real app from the live catalog.
 * Under `next dev` only (a 404 in production builds, never indexed).
 *
 *   ?v=    now (the page's, Profile, picked 2026-11-18: components/app/developer.tsx), a (Grid, the page's first),
 *          b (Shelf), d (Byline); the others are in components/app/developer-lab/designs.tsx
 *   ?card= how Profile's card is grounded: glow (the page's), raised, outline or tint
 *   ?app= the app (Briefcase by default: two developers; Ring has one, with See all)
 * Plain links, so the page works without script, as the store does.
 */
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { notFound } from "next/navigation";
import { CARD_STYLES, type CardStyle, Developer } from "@/components/app/developer";
import { BylineDesign, GridDesign, ShelfDesign } from "@/components/app/developer-lab/designs";
import { Page } from "@/components/parts/page";
import { apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { developerWork, type DeveloperWork } from "@/lib/developers";
import type { App } from "@/lib/types";
import styles from "../app-header/lab.module.css";

export const metadata: Metadata = { title: "Developer lab", robots: { index: false, follow: false } };

type Render = (app: App, work: DeveloperWork, card: CardStyle) => ReactNode;

const CARD_LABELS: Record<CardStyle, string> = { raised: "Raised", outline: "Outline", tint: "Tint", glow: "Glow" };
const DESIGNS: { id: string; label: string; line: string; render: Render }[] = [
  { id: "now", label: "Now · Profile", line: "The page's (picked 2026-11-18), after Google Play's About the developer and GitHub's sidebar: who and where in a card, More by as a compact list beside it. Card: how the card is grounded.", render: (app, work, card) => <Developer app={app} work={work} card={card} /> },
  { id: "a", label: "A · Grid", line: "The page's first: the people as the store's app rows on its grid, the links as pills, More by as the app grid under them.", render: (app, work) => <GridDesign app={app} work={work} /> },
  { id: "b", label: "B · Shelf", line: "After the App Store: the people as chips, the links as outlined pills, More by as a sideways shelf of tiles (a big icon, the name, two lines).", render: (app, work) => <ShelfDesign app={app} work={work} /> },
  { id: "d", label: "D · Byline", line: "After Raycast's store: one sentence for who made it, the links as words, More by as a row of icons with their names.", render: (app, work) => <BylineDesign app={app} work={work} /> },
];

const APPS = [
  { id: "briefcase", label: "Briefcase (two developers)" },
  { id: "ring", label: "Ring (one)" },
  { id: "omni", label: "Omni (custom links)" },
  { id: "lucent", label: "Lucent (no links)" },
];

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

export default async function DeveloperLab({ searchParams }: Props) {
  if (process.env.NODE_ENV === "production") notFound();
  const params = await searchParams;
  const design = DESIGNS.find(item => item.id === params.v) ?? DESIGNS[0];
  const appId = typeof params.app === "string" && APP_ID_PATTERN.test(params.app) ? params.app : "briefcase";
  const app = await apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`).catch(() => null);
  const work = app ? await developerWork(app) : { totals: {}, more: [] };
  const card = CARD_STYLES.find(item => item === params.card) ?? "glow";
  const href = (next: { v?: string; app?: string; card?: string }) => `/playground/developer?${new URLSearchParams({ v: next.v ?? design.id, app: next.app ?? appId, card: next.card ?? card })}`;

  return (
    <Page wide>
      <div className={styles.lab}>
        <h1 className="sr-only">Developer lab</h1>
        <div className={styles.controls}>
          <nav className={styles.tabs} aria-label="Designs">
            {DESIGNS.map(item => <a key={item.id} href={href({ v: item.id })} className={styles.tab} aria-current={item.id === design.id ? "page" : undefined}>{item.label}</a>)}
          </nav>
          <nav className={styles.apps} aria-label="Apps">
            {APPS.map(item => <a key={item.id} href={href({ app: item.id })} className={styles.app} aria-current={item.id === appId ? "page" : undefined}>{item.label}</a>)}
          </nav>
        </div>
        <p className={styles.line}>{design.line}</p>
        {design.id === "now" ? (
          <nav className={styles.apps} aria-label="How the card is grounded">
            <span className={styles.line}>Card:</span>
            {CARD_STYLES.map(item => <a key={item} href={href({ card: item })} className={styles.app} aria-current={item === card ? "page" : undefined}>{CARD_LABELS[item]}</a>)}
          </nav>
        ) : null}
        <hr className={styles.rule} />
        {app ? design.render(app, work, card) : <p className={styles.line}>The catalog has no app “{appId}” (or isn’t answering).</p>}
      </div>
    </Page>
  );
}
