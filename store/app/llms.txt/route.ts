/**
 * GET /llms.txt (llmstxt.org): Silicon Apps for language models, the Carbon's own store/llms/llms.md served exactly as
 * written (bundled at build time by scripts/build-llms.mjs; nothing is read at run time).
 */
import { LLMS_TXT, LLMS_TXT_MODIFIED } from "@/lib/agent/generated/llms";
import { errorResponse, preflight, publicResponse } from "@/lib/server/public-response";

export function GET(request: Request) {
  if (LLMS_TXT === null) return errorResponse(404, { code: "not_found", message: "This build has no llms.txt.", hint: "Read https://developers.teamofsilicons.com/llms.txt instead." });
  return publicResponse(request, LLMS_TXT, { type: "text/plain; charset=utf-8", maxAge: 3600, modified: LLMS_TXT_MODIFIED });
}

export const HEAD = GET;
export const OPTIONS = () => preflight();
