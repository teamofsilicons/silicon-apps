/**
 * /robots.txt: every public page and agent file is open to every crawler, and the crawlers of answer engines and
 * agents are named and welcome (the same list as the developer site, developer/lib/agent/robots.ts). Sign-in and
 * sign-out, Settings, typed search results, the API (for programs, not crawlers) and the MCP endpoint are kept out.
 * App media (logos, banners, screenshots) stays open so pages show with their pictures, and so do the discovery
 * documents an agent needs: /llms.txt, /llms-full.txt, /openapi.json and /.well-known/*.
 */
import { CANONICAL_ORIGIN } from "@/lib/site";

/** Crawlers that read for answer engines and agents. They are welcome to everything public here. */
export const AI_CRAWLERS = [
  "GPTBot", "OAI-SearchBot", "ChatGPT-User",
  "ClaudeBot", "Claude-User", "Claude-SearchBot", "anthropic-ai",
  "PerplexityBot", "Perplexity-User",
  "Google-Extended", "Applebot-Extended", "Meta-ExternalAgent", "Amazonbot", "DuckAssistBot",
  "CCBot", "cohere-ai", "MistralAI-User",
];

export const ALLOWED = ["/", "/v1/apps/*/media/"];
export const DISALLOWED = ["/sign-in", "/sign-out", "/settings", "/search?q=", "/v1/", "/mcp", "/api/"];

export function robotsTxt(): string {
  return [
    "# apps.teamofsilicons.com: Silicon Apps, the app store of the Silicon ecosystem.",
    "# Every app page, author page and agent file here is meant to be read, quoted and used by Carbons and Silicons alike.",
    "# Search engines, answer engines and agents are all welcome, and named below.",
    "# Agents: start with /llms.txt (or /llms-full.txt), /.well-known/agent.json, /openapi.json and the MCP server at /mcp.",
    "",
    ...AI_CRAWLERS.map(agent => `User-agent: ${agent}`),
    "User-agent: *",
    ...ALLOWED.map(path => `Allow: ${path}`),
    ...DISALLOWED.map(path => `Disallow: ${path}`),
    "",
    `Sitemap: ${CANONICAL_ORIGIN}/sitemap.xml`,
    "",
  ].join("\n");
}
