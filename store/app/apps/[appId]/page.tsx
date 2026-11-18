/**
 * /apps/{app_id}: one app, in the Sidebar layout (components/app/app-layout.tsx, Anil, 2026-11-18, picked from
 * /playground/app-page: the short header, a reading column, and a sidebar with the install command, the facts and the
 * developer card; More by at the foot). Cleared to be designed again one element at a time (Anil, 2026-11-18); so far the header
 * (components/app/app-header.tsx: icon, name, tagline and the actions, then the facts), the
 * screenshots (components/app/screenshots.tsx: a sideways strip, desktop frames 16:10 and mobile 9:19.5; none yet in
 * the catalog, so see them in /playground/app-header) and About (components/app/about.tsx: the description and tags) and What's new
 * (components/app/whats-new.tsx, Store: the version and how long ago, the notes cut to three lines with more, then
 * folds for version history, withdrawn releases and other ways to install; the history from lib/releases.ts) and
 * Compatibility (components/app/compatibility.tsx: a row per system with every processor as a chip, built or not, and
 * the developer's reason for a missing required one) and Developer (components/app/developer.tsx: the authors, the
 * links and more by them, from lib/developers.ts) and Ratings and reviews (components/app/reviews.tsx, with the review form; its server actions in actions.ts). The
 * page is wide (Anil, 2026-11-18: the overall alignment), so it starts on the same left edge as All Apps and a category. The page before this (cover, Try it, facts, About,
 * details, reviews) is in git at e7748ac, and its styles (components/app/app-page.module.css) and the copy-command
 * button (components/app/command.tsx) are kept for reuse. Server-rendered; apps the visitor may not see answer 404
 * (proxy.ts asks the API first, and this page asks again).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { About } from "@/components/app/about";
import { SidebarLayout } from "@/components/app/app-layout";
import { Compatibility } from "@/components/app/compatibility";
import { Reviews } from "@/components/app/reviews";
import { Screenshots } from "@/components/app/screenshots";
import { WhatsNew } from "@/components/app/whats-new";
import { Page } from "@/components/parts/page";
import { api, apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { developerWork } from "@/lib/developers";
import { releaseHistory } from "@/lib/releases";
import { appPath, pageMetadata } from "@/lib/seo";
import { getAccount } from "@/lib/session";
import { installCommand } from "@/lib/site";
import type { App, Reviews as ReviewList } from "@/lib/types";

type Props = { params: Promise<{ appId: string }>; searchParams?: Promise<Record<string, string | string[] | undefined>> };

async function load(appId: string): Promise<App | null> {
  if (!APP_ID_PATTERN.test(appId)) return null;
  return apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`);
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { appId } = await params;
  const app = await load(appId).catch(() => null);
  if (!app) return { title: { absolute: "App not found · Silicon Apps" }, robots: { index: false, follow: true } };
  return pageMetadata({
    title: app.name,
    description: app.description || `${app.name} on Silicon Apps. Install it with ${installCommand(app.app_id)}.`,
    path: appPath(app.app_id),
    index: app.visibility === "public",
  });
}

export default async function AppPage({ params, searchParams }: Props) {
  const { appId } = await params;
  const query = (await searchParams) ?? {};
  const status = { review: typeof query.review === "string" ? query.review : undefined, code: typeof query.code === "string" ? query.code : undefined };
  const app = await load(appId);
  if (!app || app.app_id !== appId) notFound();
  const [history, work, reviews, account] = await Promise.all([
    releaseHistory(app),
    developerWork(app),
    api<ReviewList>(`/v1/apps/${encodeURIComponent(app.app_id)}/reviews`).catch((): ReviewList => ({ items: [], rating: null, count: 0 })),
    getAccount(),
  ]);
  return (
    <article aria-labelledby="app-name">
      <Page wide>
        <SidebarLayout app={app} work={work}>
          <Screenshots app={app} />
          <About app={app} />
          <WhatsNew app={app} history={history} />
          <Compatibility app={app} />
          <Reviews app={app} reviews={reviews} account={account} status={status} />
        </SidebarLayout>
      </Page>
    </article>
  );
}
