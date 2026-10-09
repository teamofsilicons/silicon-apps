/**
 * Server-side settings of the store, read from the environment at request time (never baked into the build), so one
 * build serves every stack.
 *
 *   APPS_API_URL       where this server reaches the Apps API, server to server   [http://127.0.0.1:4310]
 *   STORE_PUBLIC_URL   the origin browsers use for this store: the Origin sent with the visitor's writes (the API
 *                      checks it against APPS_ALLOWED_ORIGINS) and the same-origin check of the store's own forms
 *                      [https://apps.teamofsilicons.com]
 *
 * Canonical links, Open Graph, JSON-LD and the sitemap always name the production origin (lib/site.ts).
 */
import { CANONICAL_ORIGIN } from "./site";

const trim = (value: string) => value.trim().replace(/\/+$/, "");

export function appsApiUrl(): string {
  return trim(process.env.APPS_API_URL || "http://127.0.0.1:4310");
}

export function siteOrigin(): string {
  const value = process.env.STORE_PUBLIC_URL;
  try {
    return new URL(value?.trim() || CANONICAL_ORIGIN).origin;
  } catch {
    return CANONICAL_ORIGIN;
  }
}

/** Cookies the store sets itself are Secure when the store is served over https. */
export function secureCookies(): boolean {
  return siteOrigin().startsWith("https://");
}

/** An absolute URL on the canonical origin: "/" and "/apps/ring" become https://apps.teamofsilicons.com/… */
export function absolute(path: string): string {
  return path === "/" ? `${CANONICAL_ORIGIN}/` : `${CANONICAL_ORIGIN}${path.startsWith("/") ? path : `/${path}`}`;
}
