/** GET /sitemap.xml (lib/agent/sitemap.ts), read from the catalog on each request (cached for 15 minutes). */
import { sitemapXml } from "@/lib/agent/sitemap";
import { errorResponse, publicResponse } from "@/lib/server/public-response";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  let body: string;
  try {
    body = await sitemapXml();
  } catch {
    return errorResponse(503, { code: "catalog_unavailable", message: "The catalog is not answering, so the sitemap cannot be built right now.", hint: "Try again in a minute." }, { "Retry-After": "60" });
  }
  return publicResponse(request, body, { type: "application/xml; charset=utf-8", maxAge: 900 });
}
