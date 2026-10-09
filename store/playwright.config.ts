/**
 * The store's e2e suite: the built standalone server against a local fixture Apps API, as production runs it (Caddy's
 * API routes are stood in for by STORE_DEV_API_PASSTHROUGH=1). Both servers start here and stop when the run ends:
 *
 *   CARGO_TARGET_DIR=target/integration cargo build -p silicon-apps-server --bin apps-server   (repository root)
 *   pnpm build && pnpm test:e2e                                                                  (store/)
 *
 * E2E_STORE_PORT and E2E_API_PORT pick the ports (8710 and 4510). E2E_DEV=1 runs `next dev` instead of the build.
 * Servers already listening on those ports are reused outside CI (the suite works against a used fixture too).
 */
import { defineConfig } from "@playwright/test";

const storePort = Number(process.env.E2E_STORE_PORT || 8710);
const apiPort = Number(process.env.E2E_API_PORT || 4510);
const store = `http://127.0.0.1:${storePort}`;
const api = `http://127.0.0.1:${apiPort}`;
const storeEnv = { PORT: String(storePort), HOSTNAME: "127.0.0.1", APPS_API_URL: api, STORE_PUBLIC_URL: store, STORE_DEV_API_PASSTHROUGH: "1" };

export default defineConfig({
  testDir: "./e2e",
  timeout: 45_000,
  expect: { timeout: 10_000 },
  fullyParallel: true,
  workers: 3,
  forbidOnly: Boolean(process.env.CI),
  reporter: process.env.CI ? "line" : [["list"]],
  use: { baseURL: store, headless: true, viewport: { width: 1440, height: 1000 }, trace: "retain-on-failure" },
  webServer: [
    {
      command: "node scripts/e2e-api.mjs",
      url: `${api}/health`,
      env: { E2E_API_PORT: String(apiPort), E2E_STORE_URL: store },
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
      stdout: "pipe",
    },
    {
      command: process.env.E2E_DEV ? `pnpm dev` : "node .next/standalone/server.js",
      url: `${store}/robots.txt`,
      env: storeEnv,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
