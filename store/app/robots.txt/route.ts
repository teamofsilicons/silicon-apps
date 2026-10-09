/** GET /robots.txt (lib/agent/robots.ts). */
import { robotsTxt } from "@/lib/agent/robots";
import { publicResponse } from "@/lib/server/public-response";

export function GET(request: Request) {
  return publicResponse(request, robotsTxt(), { type: "text/plain; charset=utf-8", maxAge: 86400 });
}
