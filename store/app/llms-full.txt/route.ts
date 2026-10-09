/**
 * GET /llms-full.txt (llmstxt.org): the store's agent file in one piece. The store has one hand-written file,
 * store/llms/llms.md, and it is already complete, so /llms-full.txt serves it exactly as written, as /llms.txt does
 * (bundled at build time by scripts/build-llms.mjs). Making apps and adding sign-in are in the developer portal's own
 * llms-full.txt.
 */
import { LLMS_TXT, LLMS_TXT_MODIFIED } from "@/lib/agent/generated/llms";
import { errorResponse, preflight, publicResponse } from "@/lib/server/public-response";

export function GET(request: Request) {
  if (LLMS_TXT === null) return errorResponse(404, { code: "not_found", message: "This build has no llms-full.txt.", hint: "Read https://developers.teamofsilicons.com/llms-full.txt instead." });
  return publicResponse(request, LLMS_TXT, { type: "text/plain; charset=utf-8", maxAge: 3600, modified: LLMS_TXT_MODIFIED });
}

export const HEAD = GET;
export const OPTIONS = () => preflight();
