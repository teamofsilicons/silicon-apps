/**
 * GET /apps/{app_id}/og.png: an app's Open Graph image with its logo, name, description and install command (lib/og.tsx).
 * Public apps only; anything else (unknown, private, a draft) gets the store's own image.
 */
import { apiOrNull, APP_ID_PATTERN } from "@/lib/api";
import { appImage } from "@/lib/og";
import type { App } from "@/lib/types";

export async function GET(_request: Request, { params }: { params: Promise<{ appId: string }> }) {
  const { appId } = await params;
  const app = APP_ID_PATTERN.test(appId) ? await apiOrNull<App>(`/v1/apps/${encodeURIComponent(appId)}`, { anonymous: true }).catch(() => null) : null;
  // A relative Location: behind Caddy the request's own URL names the loopback listener, not the public origin.
  if (!app || app.visibility !== "public") return new Response(null, { status: 307, headers: { Location: "/og.png", "Cache-Control": "public, max-age=300" } });
  return appImage(app);
}
