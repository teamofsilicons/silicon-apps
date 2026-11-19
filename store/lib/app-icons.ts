/**
 * Anil's app icons for the Team of Silicons apps, as the account site uses them (silicon-accounts web,
 * components/account/parts/app-icons.ts and public/icons/<app>/ios/AppIcon@3x.png; copied 2026-11-14 as
 * public/icons/<app>.png, the 180px iOS square, full bleed, so the mark's squircle clips it like an app icon). Where an
 * app has one, it wins over the logo in the catalog.
 */
import type { App } from "./types";

const ICONS: Record<string, string> = {
  briefcase: "/icons/briefcase.png",
  dm: "/icons/dm.png",
  ting: "/icons/ting.png",
  write: "/icons/write.png",
  "silicon-writings": "/icons/writings.png",
};

/** The app with Anil's icon as its logo, when there is one. */
export function withAppIcon<T extends Pick<App, "app_id" | "logo">>(app: T): T {
  const icon = ICONS[app.app_id];
  return icon ? { ...app, logo: icon } : app;
}
