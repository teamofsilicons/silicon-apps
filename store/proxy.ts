/**
 * Next 16 proxy, run before every page and route of the store.
 *
 * - Old store addresses keep working: /store and /store?q= go to / and /search, /store/{app_id} to /apps/{app_id},
 *   /apps/apps to /apps/silicon-apps, /developer/* and /docs/* to the shared developer portal (Caddy answers those
 *   first in production; these are the same redirects for any other front).
 * - A fresh CSP nonce per request, as on the developer site: script-src 'self' 'nonce-…' 'strict-dynamic'. Next puts
 *   the nonce on its own scripts; the root layout puts it on the theme boot script and the JSON-LD.
 *   No other inline script runs.
 * - x-store-path tells the layout which page it is rendering (the header marks it, and sign-in comes back to it).
 * - An app or author page the visitor cannot see is answered by the root not-found page (a 404 rendered in full on the
 *   server): one quick read of the API first, with the visitor's own session. A page that calls notFound() itself
 *   would only send an empty document for script to fill in, and the store must work without script.
 * - One sanitized page_view telemetry event per page, unless the visitor opted out in Settings or is a crawler.
 * - Local work only: under `next dev`, or with STORE_DEV_API_PASSTHROUGH=1 (the e2e suite sets it on the standalone
 *   server), the API's own paths are forwarded to APPS_API_URL, standing in for Caddy: /v1/* (the SSE streams
 *   included), /health, /openapi.json and the API's /.well-known documents. In production Caddy sends those paths to
 *   the API before they reach the store, so production never sets it.
 */
import { NextResponse, type NextRequest } from "next/server";

const isDev = process.env.NODE_ENV === "development";
const DEVELOPERS = () => (process.env.DEVELOPERS_URL?.trim() || "https://developers.teamofsilicons.com").replace(/\/+$/, "");
const API = () => (process.env.APPS_API_URL?.trim() || "http://127.0.0.1:4310").replace(/\/+$/, "");
/** Answered by the Apps API, never by the store (Caddy routes them in production). */
const API_PATHS = /^\/(v1(\/.*)?|health|openapi\.json|\.well-known\/(agent|agent-card|silicon-apps-keys)\.json)$/;
const passthrough = () => isDev || process.env.STORE_DEV_API_PASSTHROUGH === "1";
const CRAWLER = /bot|crawl|spider|slurp|curl|wget|python|httpx|go-http|java\/|headless|lighthouse|preview|facebookexternalhit|embedly|monitor/i;

function contentSecurityPolicy(nonce: string): string {
  return [
    "default-src 'self'",
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'${isDev ? " 'unsafe-eval'" : ""}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' https: data: blob:",
    "media-src 'self' https: blob:",
    "font-src 'self' data:",
    "connect-src 'self'",
    "object-src 'none'",
    "frame-ancestors 'none'",
    "form-action 'self'",
    "base-uri 'none'",
    "manifest-src 'self'",
  ].join("; ");
}

function redirect(request: NextRequest, location: string): NextResponse {
  return NextResponse.redirect(new URL(location, request.url), 308);
}

/** The redirects for addresses the old single-page store answered. */
function legacy(request: NextRequest): NextResponse | null {
  const { pathname, search, searchParams } = request.nextUrl;
  if (pathname === "/store" || pathname === "/store/") {
    const params = new URLSearchParams();
    for (const key of ["q", "visibility", "tag", "target"]) {
      const value = searchParams.get(key);
      if (value && !(key === "visibility" && value === "all")) params.set(key, value);
    }
    const query = params.toString();
    return redirect(request, query ? `/search?${query}` : "/");
  }
  if (pathname === "/store/apps" || pathname === "/store/apps/") return redirect(request, "/apps/silicon-apps");
  const storeApp = /^\/store\/([^/]+)\/?$/.exec(pathname);
  if (storeApp) return redirect(request, `/apps/${storeApp[1]}${search}`);
  if (pathname === "/apps" || pathname === "/apps/") return redirect(request, `/search${search}`);
  if (/^\/apps\/apps(\/|$)/.test(pathname)) return redirect(request, pathname.replace(/^\/apps\/apps/, "/apps/silicon-apps") + search);
  if (pathname === "/developer" || pathname.startsWith("/developer/")) {
    const rest = pathname.replace(/^\/developer\/?/, "");
    const app = /^apps\/([a-z0-9_-]+)\/?$/.exec(rest);
    if (app) {
      const params = new URLSearchParams(search);
      const tab = params.get("tab");
      const section = tab && ["releases", "authors", "history"].includes(tab) ? tab : "publishing";
      params.delete("tab");
      const query = params.toString();
      return NextResponse.redirect(`${DEVELOPERS()}/apps/${app[1]}/${section}${query ? `?${query}` : ""}`, 308);
    }
    return NextResponse.redirect(`${DEVELOPERS()}/${rest}${search}`, 308);
  }
  if (pathname === "/docs" || pathname.startsWith("/docs/")) {
    const rest = pathname.replace(/^\/docs\/?/, "");
    return NextResponse.redirect(`${DEVELOPERS()}/docs/apps${rest ? `/${rest}` : ""}${search}`, 308);
  }
  return null;
}

const APP_PAGE = /^\/apps\/([^/]+)\/?$/;
const AUTHOR_PAGE = /^\/authors\/([^/]+)\/?$/;

/** True when the API says this app or author page has nothing the visitor may see (404). Anything else renders. */
async function hidden(request: NextRequest): Promise<boolean> {
  if ((request.method !== "GET" && request.method !== "HEAD") || request.headers.get("rsc")) return false;
  const { pathname } = request.nextUrl;
  const app = APP_PAGE.exec(pathname);
  const author = app ? null : AUTHOR_PAGE.exec(pathname);
  if (!app && !author) return false;
  let id: string;
  try {
    id = decodeURIComponent((app ?? author)![1]);
  } catch {
    return true;
  }
  if (!/^[A-Za-z0-9_.:-]{1,100}$/.test(id)) return true;
  const headers: Record<string, string> = { Accept: "application/json", "User-Agent": "silicon-apps-store" };
  const session = request.cookies.get("apps_session")?.value;
  if (session && /^[A-Za-z0-9_-]{8,200}$/.test(session)) headers.Cookie = `apps_session=${session}`;
  const forwarded = request.headers.get("x-forwarded-for")?.split(",")[0]?.trim();
  if (forwarded && /^[0-9a-fA-F:.]{2,45}$/.test(forwarded)) headers["X-Forwarded-For"] = forwarded;
  if (request.cookies.get("apps_telemetry")?.value === "off") headers["X-Apps-Telemetry"] = "off";
  try {
    const path = app ? `/v1/apps/${encodeURIComponent(id)}` : `/v1/authors/${encodeURIComponent(id)}?limit=1`;
    const response = await fetch(`${API()}${path}`, { headers, cache: "no-store", redirect: "manual", signal: AbortSignal.timeout(5_000) });
    await response.body?.cancel();
    return response.status === 404;
  } catch {
    return false;
  }
}

function countPageView(request: NextRequest): void {
  if (request.method !== "GET" || request.cookies.get("apps_telemetry")?.value === "off") return;
  if (request.headers.get("next-router-prefetch") || request.headers.get("purpose") === "prefetch") return;
  const accept = request.headers.get("accept") || "";
  if (!accept.includes("text/html") && request.headers.get("rsc") !== "1") return;
  if (CRAWLER.test(request.headers.get("user-agent") || "")) return;
  const { pathname } = request.nextUrl;
  if (/\.(txt|xml|json|png|svg|ico|webmanifest)$/.test(pathname) || pathname === "/mcp") return;
  void fetch(`${API()}/v1/telemetry`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json", "Idempotency-Key": crypto.randomUUID() },
    body: JSON.stringify({ source: "silicon-apps.store", step: "store.page_view", progress: "complete", event: "page_view", path: pathname }),
    signal: AbortSignal.timeout(3_000),
  }).catch(() => undefined);
}

export async function proxy(request: NextRequest) {
  const { pathname, search } = request.nextUrl;

  if (passthrough() && API_PATHS.test(pathname)) {
    return NextResponse.rewrite(new URL(`${API()}${pathname}${search}`));
  }

  const moved = legacy(request);
  if (moved) return moved;

  const nonce = btoa(crypto.randomUUID());
  const csp = contentSecurityPolicy(nonce);
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("x-store-path", `${pathname}${search}`);
  requestHeaders.set("Content-Security-Policy", csp);

  countPageView(request);

  // An unmatched address renders the root not-found page in full, with a 404; the visitor keeps the URL they asked for.
  const response = (await hidden(request))
    ? NextResponse.rewrite(new URL(`/__not-found${pathname}`, request.url), { request: { headers: requestHeaders } })
    : NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set("Content-Security-Policy", csp);
  response.headers.set("X-Frame-Options", "DENY");
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  return response;
}

export const config = {
  matcher: [
    {
      // Static files set their own headers; everything else (pages, route handlers, the API passthrough) comes here.
      source: "/((?!_next/static|_next/image|fonts/|favicon\\.ico|icon|apple-touch-icon|og\\.png).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
};
