/**
 * Next.js config for apps.teamofsilicons.com, the Silicon Apps store.
 *
 * In production Caddy sits in front: /v1/* (the SSE streams included), /health, /openapi.json,
 * /.well-known/agent.json, /.well-known/agent-card.json, /.well-known/silicon-apps-keys.json and the installers go to
 * the Apps API or straight from disk, and every other path comes here. The store server calls the API itself
 * (APPS_API_URL, read at request time), so one build serves any stack. For local work without Caddy, `next dev` (or
 * STORE_DEV_API_PASSTHROUGH=1 on a built server) makes proxy.ts forward the API's paths to APPS_API_URL.
 *
 * `output: "standalone"` builds a self-contained server: `pnpm build` copies public/ and .next/static into
 * .next/standalone (scripts/standalone.mjs), which then runs with `node server.js` (PORT, HOSTNAME).
 */
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { NextConfig } from "next";

const root = dirname(fileURLToPath(import.meta.url));

const nextConfig: NextConfig = {
  output: "standalone",
  reactStrictMode: true,
  poweredByHeader: false,
  // Keep the listener's exact origin when the proxy rewrites (127.0.0.1 is otherwise normalized to localhost).
  skipProxyUrlNormalize: true,
  devIndicators: false,
  // AGENTS.md is written by hand; `next dev` must not create one.
  agentRules: false,
  turbopack: { root },
  outputFileTracingRoot: root,
  // Read from disk at run time by the Open Graph image routes (the llms files are bundled at build time instead).
  outputFileTracingIncludes: { "/*": ["./assets/og/*"] },
  // App logos and media are shown as they are, never fetched or resized by this server.
  images: { unoptimized: true },
  allowedDevOrigins: ["127.0.0.1"],
  async headers() {
    return [{ source: "/fonts/:path*", headers: [{ key: "Cache-Control", value: "public, max-age=31536000, immutable" }] }];
  },
};

export default nextConfig;
