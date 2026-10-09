/** Helpers for the store's own POST routes (sign-out, theme, telemetry): same-origin checks and safe return paths. */
import type { NextRequest } from "next/server";
import { siteOrigin } from "./config";

/** The request's own origin as the browser saw it (Caddy keeps the Host header). */
function requestOrigin(request: NextRequest): string {
  const host = request.headers.get("x-forwarded-host") || request.headers.get("host") || "";
  const proto = request.headers.get("x-forwarded-proto") || request.nextUrl.protocol.replace(":", "");
  return `${proto}://${host}`;
}

/** A form post must come from this site: its Origin is ours, or (older browsers) Sec-Fetch-Site says same-origin. */
export function sameOrigin(request: NextRequest): boolean {
  const origin = request.headers.get("origin");
  if (origin) return origin === siteOrigin() || origin === requestOrigin(request);
  return request.headers.get("sec-fetch-site") === "same-origin";
}

/** Only a local path that starts with one slash, so a form can never send the visitor to another site. */
export function localPath(value: unknown, fallback = "/"): string {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim();
  if (!trimmed.startsWith("/") || trimmed.startsWith("//") || /[\\\r\n\t]/.test(trimmed) || trimmed.length > 2000) return fallback;
  return trimmed;
}

/** A 303 to a local path on the URL the browser used, so the next request is a GET. */
export function seeOther(path: string): Response {
  return new Response(null, { status: 303, headers: { Location: path, "Cache-Control": "no-store" } });
}

export function forbidden(): Response {
  return Response.json(
    { error: { code: "origin_mismatch", message: "This form was not posted from the Silicon Apps store.", hint: "Open the page on the store and submit the form there." } },
    { status: 403, headers: { "Cache-Control": "no-store" } },
  );
}
