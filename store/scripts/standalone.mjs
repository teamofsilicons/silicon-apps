#!/usr/bin/env node
/**
 * Completes the standalone server after `next build` (pnpm build runs it): Next leaves public/ and .next/static out of
 * .next/standalone on purpose, so they are copied in here. The result is one directory that runs on its own:
 *
 *   cd .next/standalone && PORT=8710 HOSTNAME=127.0.0.1 node server.js
 *
 * It also checks that the files route handlers read from disk at run time were traced into it (the Open Graph fonts),
 * so a broken build fails here and not on the first request.
 */
import { cpSync, existsSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const standalone = join(root, ".next", "standalone");

if (!existsSync(join(standalone, "server.js"))) {
  console.error("standalone: .next/standalone/server.js is missing. Run `next build` first (next.config.ts sets output: \"standalone\").");
  process.exit(1);
}

for (const [from, to] of [
  [join(root, "public"), join(standalone, "public")],
  [join(root, ".next", "static"), join(standalone, ".next", "static")],
]) {
  rmSync(to, { recursive: true, force: true });
  cpSync(from, to, { recursive: true });
}

const required = ["assets/og/BDOGrotesk-Regular.ttf", "assets/og/BDOGrotesk-Medium.ttf", "assets/og/BDOGrotesk-DemiBold.ttf", "public/og.png", ".next/static"];
const missing = required.filter(path => !existsSync(join(standalone, path)));
if (missing.length) {
  console.error(`standalone: missing from .next/standalone: ${missing.join(", ")}`);
  process.exit(1);
}
console.log("standalone: .next/standalone is ready. Run it with: cd .next/standalone && node server.js");
