/**
 * POST /sign-out: revokes the Apps session through the API (which revokes the Silicon Accounts token too), clears
 * the session cookie and goes back. A plain form post, so it works without JavaScript.
 */
import type { NextRequest } from "next/server";
import { appsApiUrl, secureCookies, siteOrigin } from "@/lib/config";
import { forbidden, localPath, sameOrigin, seeOther } from "@/lib/forms";

const CLEAR = "apps_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

export async function POST(request: NextRequest) {
  if (!sameOrigin(request)) return forbidden();
  const form = await request.formData().catch(() => null);
  const back = localPath(form?.get("return_to"), "/");
  const session = request.cookies.get("apps_session")?.value;
  if (session && /^[A-Za-z0-9_-]{8,200}$/.test(session)) {
    await fetch(`${appsApiUrl()}/v1/auth/logout`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json", Cookie: `apps_session=${session}`, Origin: siteOrigin() },
      body: "{}",
      cache: "no-store",
      signal: AbortSignal.timeout(10_000),
    }).catch(() => undefined);
  }
  const response = seeOther(back);
  const secure = secureCookies() ? "; Secure" : "";
  response.headers.append("Set-Cookie", CLEAR + secure);
  return response;
}

export function GET() {
  return seeOther("/settings");
}
