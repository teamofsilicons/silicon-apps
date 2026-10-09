import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    // Internal links are plain <a> on purpose: every navigation is a full, server-rendered document (fresh header,
    // fresh CSP nonce, works with no JavaScript), as close to plain HTML as the store can be.
    rules: { "@next/next/no-html-link-for-pages": "off" },
  },
  globalIgnores([".next/**", "out/**", "next-env.d.ts", "public/**", "test-results/**", "playwright-report/**"]),
]);
