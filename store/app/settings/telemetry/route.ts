/**
 * POST /settings/telemetry: usage telemetry is on by default; "off" stores apps_telemetry=off, which stops the store's
 * page view events and is forwarded to the API as X-Apps-Telemetry: off on every call made for this browser.
 */
import type { NextRequest } from "next/server";
import { secureCookies } from "@/lib/config";
import { forbidden, sameOrigin, seeOther } from "@/lib/forms";

export async function POST(request: NextRequest) {
  if (!sameOrigin(request)) return forbidden();
  const form = await request.formData().catch(() => null);
  const off = String(form?.get("telemetry") ?? "") === "off";
  const response = seeOther("/settings?saved=telemetry");
  const secure = secureCookies() ? "; Secure" : "";
  response.headers.append("Set-Cookie", off ? `apps_telemetry=off; Path=/; Max-Age=31536000; SameSite=Lax${secure}` : `apps_telemetry=; Path=/; Max-Age=0; SameSite=Lax${secure}`);
  return response;
}
