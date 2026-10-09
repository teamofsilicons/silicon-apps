#!/usr/bin/env node
/**
 * Starts a LOCAL Apps API for the store's e2e suite and screenshots: a fresh fixture data directory, the API built
 * from this checkout, the fixture catalog (scripts/seed-fixture.mjs), then the API again so it signs the seeded
 * releases on startup, as a deployed API signs what it serves. It stays in the foreground until it is stopped
 * (Playwright's webServer stops it; so does Ctrl-C or SIGTERM), and stops the API with it.
 *
 *   node scripts/e2e-api.mjs
 *
 * Build the API first, from the repository root:
 *   CARGO_TARGET_DIR=target/integration cargo build -p silicon-apps-server --bin apps-server
 *
 * Environment (all optional):
 *   E2E_API_PORT     the API's port (setup briefly uses the next one)  [4510]
 *   E2E_STORE_URL    the store's origin the API trusts for writes     [http://127.0.0.1:8710]
 *   E2E_API_BIN      the apps-server binary                           [../target/integration/debug/apps-server]
 *   E2E_DATA_DIR     the fixture data directory (wiped on start)      [../.dev/store-e2e-fixture]
 *
 * The API runs with APPS_DEV_AUTH=1 (fixture accounts, loopback only) and a stand-in Accounts address, so
 * /v1/auth/login answers with a redirect to Accounts without any real Accounts deployment. Never point this at a
 * real data directory: the directory is deleted and re-created.
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const store = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = resolve(store, "..");
const port = Number(process.env.E2E_API_PORT || 4510);
const storeUrl = (process.env.E2E_STORE_URL || "http://127.0.0.1:8710").replace(/\/+$/, "");
const bin = resolve(process.env.E2E_API_BIN || join(repo, "target", "integration", "debug", "apps-server"));
const dataDir = resolve(process.env.E2E_DATA_DIR || join(repo, ".dev", "store-e2e-fixture"));

if (!/fixture|e2e/.test(dataDir)) {
  console.error(`e2e-api: refusing to wipe ${dataDir}; the path must contain "fixture" or "e2e".`);
  process.exit(2);
}
if (!existsSync(bin)) {
  console.error(`e2e-api: ${bin} is missing. Build it from the repository root with:\n  CARGO_TARGET_DIR=target/integration cargo build -p silicon-apps-server --bin apps-server`);
  process.exit(2);
}

const env = {
  ...process.env,
  APPS_DEV_AUTH: "1",
  APPS_BIND: `127.0.0.1:${port}`,
  APPS_PUBLIC_URL: storeUrl,
  APPS_ALLOWED_ORIGINS: storeUrl,
  APPS_IMPORT_ACCOUNTS: "0",
  APPS_DATA_DIR: dataDir,
  // A stand-in: sign-in only needs to build the authorize redirect, never to reach Accounts.
  APPS_ACCOUNTS_URL: "http://127.0.0.1:4599",
  APPS_ACCOUNTS_APP_SECRET: "e2e-stand-in-secret",
  APPS_RATE_LIMIT_READS_PER_MINUTE: "100000",
  APPS_RATE_LIMIT_WRITES_PER_MINUTE: "10000",
};

let child = null;
/** Starts apps-server on a port: the setup run uses a port of its own, so nothing sees the API before it is seeded. */
function start(stdio, on) {
  child = spawn(bin, [], { env: { ...env, APPS_BIND: `127.0.0.1:${on}` }, stdio });
  return child;
}

async function healthy(on, timeoutMs = 30_000) {
  const until = Date.now() + timeoutMs;
  while (Date.now() < until) {
    if (child?.exitCode !== null && child?.exitCode !== undefined) throw new Error(`apps-server exited with ${child.exitCode}`);
    try {
      const response = await fetch(`http://127.0.0.1:${on}/health`, { signal: AbortSignal.timeout(1_000) });
      if (response.ok) return;
    } catch {
      // Not listening yet.
    }
    await new Promise(done => setTimeout(done, 200));
  }
  throw new Error(`apps-server did not answer /health on port ${on} within ${timeoutMs / 1000} seconds`);
}

function stop() {
  const running = child;
  return new Promise(done => {
    if (!running || running.exitCode !== null || running.signalCode !== null) return done();
    const force = setTimeout(() => running.kill("SIGKILL"), 5_000);
    running.once("exit", () => {
      clearTimeout(force);
      done();
    });
    running.kill("SIGTERM");
  });
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    console.log(`e2e-api: ${signal}, stopping the Apps API`);
    stop().then(() => process.exit(0));
  });
}

rmSync(dataDir, { recursive: true, force: true });
mkdirSync(dataDir, { recursive: true });

// 1. Create the database, on a setup port (the next one up).
start(["ignore", "ignore", "inherit"], port + 1);
await healthy(port + 1);
// 2. Seed the fixture catalog, media and fixture browser sessions.
const seeded = spawnSync(process.execPath, [join(store, "scripts", "seed-fixture.mjs"), "--data-dir", dataDir], { cwd: store, stdio: "inherit" });
await stop();
if (seeded.status !== 0) {
  console.error("e2e-api: seeding failed");
  process.exit(1);
}
// 3. Start again: the API signs the seeded releases on startup, then serves until stopped.
start(["ignore", "inherit", "inherit"], port);
await healthy(port);
console.log(`e2e-api: the fixture Apps API is ready at http://127.0.0.1:${port} (data in ${dataDir})`);
child.on("exit", (code, signal) => {
  console.log(`e2e-api: apps-server stopped (${signal ?? `exit ${code}`})`);
  process.exit(code ?? 0);
});
