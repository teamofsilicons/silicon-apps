/**
 * GET /sign-in: starts the API's browser sign-in through Silicon Accounts and comes back to the page the visitor was
 * on (?return_to=, or the same-origin Referer). The API owns PKCE, state and the HttpOnly session cookie, and
 * signs in as the app `silicon-apps`.
 */
import type { NextRequest } from "next/server";
import { siteOrigin } from "@/lib/config";
import { localPath } from "@/lib/forms";

export function GET(request: NextRequest) {
  let target = localPath(request.nextUrl.searchParams.get("return_to"), "");
  if (!target) {
    try {
      const referer = new URL(request.headers.get("referer") || "");
      if (referer.origin === siteOrigin() || referer.host === request.headers.get("host")) target = localPath(`${referer.pathname}${referer.search}`);
    } catch {
      target = "/";
    }
  }
  if (!target || target.startsWith("/sign-in") || target.startsWith("/v1/")) target = "/";
  return new Response(null, { status: 303, headers: { Location: `/v1/auth/login?return_to=${encodeURIComponent(target)}`, "Cache-Control": "no-store" } });
}
