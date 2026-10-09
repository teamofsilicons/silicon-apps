#!/usr/bin/env node
/**
 * Seeds a LOCAL Apps API data directory with a realistic fixture catalog for store development and the e2e suite.
 * Never point it at a real data directory: it replaces the whole catalog document.
 *
 *   node scripts/seed-fixture.mjs --data-dir ../.dev/store-fixture
 *
 * The API must already have started once on that directory (it creates the database). The script writes the catalog,
 * renders PNG banners and screenshots with Playwright's Chromium into the API's media store, and adds three browser
 * sessions for APPS_DEV_AUTH=1 fixture accounts (cookie apps_session=e2e-mira-session, e2e-nova-session or
 * e2e-ada-session). Releases carry withdrawn versions and an author signature, and the API signs every inspected
 * package when it next starts (scripts/e2e-api.mjs restarts it after seeding).
 */
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { chromium } from "@playwright/test";

const args = process.argv.slice(2);
const dataDir = resolve(args[args.indexOf("--data-dir") + 1] || "");
if (!args.includes("--data-dir") || !existsSync(join(dataDir, "apps.sqlite"))) {
  console.error("Usage: node scripts/seed-fixture.mjs --data-dir DIR (start the API on DIR once first; it creates apps.sqlite)");
  process.exit(2);
}
if (!/fixture|e2e|store-dev/.test(dataDir)) {
  console.error(`Refusing to seed ${dataDir}: the path must contain "fixture", "e2e" or "store-dev" so a real catalog is never replaced.`);
  process.exit(2);
}

const TARGETS = ["linux-x86_64", "linux-i686", "linux-aarch64", "linux-armv7hf", "windows-x86_64", "windows-i686", "windows-aarch64", "macos-x86_64", "macos-aarch64"];
const day = (n) => new Date(Date.UTC(2026, 9, 9) - n * 86400000).toISOString();
const sha = (text) => createHash("sha256").update(text).digest("hex");
const people = {
  saket: { uuid: "saket", id: "c:saket", display_name: "Saket Gupta" },
  shubham: { uuid: "shubham", id: "c:shubham", display_name: "Shubham" },
  growth: { uuid: "head_of_growth", id: "si:head_of_growth", display_name: "Head of Growth" },
  mira: { uuid: "mira", id: "c:mira", display_name: "Mira Patel" },
  nova: { uuid: "nova", id: "si:nova", display_name: "Nova" },
  ada: { uuid: "ada", id: "c:ada", display_name: "Ada Okafor" },
};

function svgLogo(background, foreground, glyph) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64"><rect width="64" height="64" rx="15" fill="${background}"/>${glyph.replaceAll("FG", foreground)}</svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}

const logos = {
  ring: svgLogo("#0f172a", "#7eadf2", '<circle cx="32" cy="32" r="14" fill="none" stroke="FG" stroke-width="5"/><circle cx="32" cy="32" r="4" fill="FG"/>'),
  remind: svgLogo("#fff4e5", "#c2410c", '<circle cx="32" cy="34" r="15" fill="none" stroke="FG" stroke-width="4"/><path d="M32 26v9l6 4" stroke="FG" stroke-width="4" stroke-linecap="round" fill="none"/>'),
  dm: svgLogo("#1f5fb8", "#ffffff", '<path d="M18 22h28a4 4 0 0 1 4 4v14a4 4 0 0 1-4 4H30l-8 6v-6h-4a4 4 0 0 1-4-4V26a4 4 0 0 1 4-4z" fill="FG"/>'),
  notes: svgLogo("#ecfdf3", "#15803d", '<path d="M22 16h16l8 8v24H22z" fill="none" stroke="FG" stroke-width="4" stroke-linejoin="round"/><path d="M27 32h14M27 39h10" stroke="FG" stroke-width="4" stroke-linecap="round"/>'),
  vault: svgLogo("#111827", "#f2c14e", '<rect x="18" y="28" width="28" height="20" rx="4" fill="none" stroke="FG" stroke-width="4"/><path d="M24 28v-5a8 8 0 0 1 16 0v5" fill="none" stroke="FG" stroke-width="4"/>'),
};

/**
 * Packages for one release. `inspected` lets the API sign them when it starts (an uninspected package stays unsigned,
 * as a package uploaded before signing existed whose archive is gone). `signer` adds the author's own signature; the
 * values are stand-ins, since the store only shows who signed.
 */
function packages(appId, targets, version, { inspected = true, signer = null } = {}) {
  return targets.map((target) => ({
    id: randomUUID(),
    target,
    sha256: sha(`${appId}-${target}-${version}`),
    size: 2_400_000 + (target.length * 7919) % 900_000,
    command: appId,
    validation: ["--help", "accounts --json", "login status --json"].map((command) => ({ command, exit_code: 0, stdout: "", stderr: "", passed: true, expected: "exit 0" })),
    created_at: day(3),
    install_script: null,
    inspected,
    author_signature: signer
      ? { key_id: `ak_${sha(signer.uuid).slice(0, 16)}`, algorithm: "ed25519", public_key: Buffer.from(sha(`key-${signer.uuid}`).slice(0, 32)).toString("base64"), signature: Buffer.from(sha(`${appId}-${target}-${version}-sig`)).toString("base64"), signer_uuid: signer.uuid, signer_id: signer.id, signed_at: day(6) }
      : null,
  }));
}

function app({ app_id, name, description, tags, authors, targets = TARGETS, production, development, installs, reviews = [], logo = "", banner = "", carousel = [], links = {}, visibility = "public", access = [], created, updated, published = true, inspected = true, signer = null, withdrawn = [] }) {
  const releases = [];
  const all = [];
  // Withdrawn releases: never served again, listed on the app page with their reasons.
  for (const [channel, version, reason, ago] of withdrawn) {
    const pk = packages(app_id, targets, version, { inspected });
    all.push(...pk);
    releases.push({ id: randomUUID(), app_id, channel, version, package_ids: pk.map((p) => p.id), notes: `Version ${version}.`, created_at: day(ago + 1), promoted_from: null, withdrawn: { at: day(ago), by_uuid: authors[0].uuid, by_id: authors[0].id, reason } });
  }
  if (development) {
    const pk = packages(app_id, targets, development, { inspected });
    all.push(...pk);
    releases.push({ id: randomUUID(), app_id, channel: "development", version: development, package_ids: pk.map((p) => p.id), notes: `Development build ${development}: the next set of improvements, ready to try.`, created_at: day(2), promoted_from: null });
  }
  if (production) {
    const pk = packages(app_id, targets, production, { inspected, signer });
    all.push(...pk);
    releases.push({ id: randomUUID(), app_id, channel: "production", version: production, package_ids: pk.map((p) => p.id), notes: `Version ${production} with faster startup and clearer errors.`, created_at: day(6), promoted_from: null });
  }
  return {
    app_id, name, description, logo, logo_alt: logo ? `${name} logo` : "", banner, banner_alt: banner ? `${name} banner` : "", tags, visibility,
    domains: [], account_ids: access.map((uuid) => Object.values(people).find((p) => p.uuid === uuid)?.id ?? uuid), access_uuids: access,
    links, carousel, published, setup_step: published ? 7 : 2, created_at: created, updated_at: updated,
    authors: authors.map((p, i) => ({ ...p, joined_at: day(200 - i) })), admin_uuid: authors[0].uuid,
    packages: all, releases, reviews: reviews.map(([who, rating, text, ago]) => ({ uuid: who.uuid, id: who.id, rating, text, updated_at: day(ago) })),
    installs, history: [], secret_hash: "",
  };
}

async function renderMedia() {
  const browser = await chromium.launch();
  const page = await browser.newPage({ deviceScaleFactor: 1 });
  const shots = {};
  async function shot(key, appId, width, height, html) {
    await page.setViewportSize({ width, height });
    await page.setContent(`<!doctype html><html><body style="margin:0;font-family:-apple-system,system-ui,sans-serif">${html}</body></html>`);
    const png = await page.screenshot({ type: "png" });
    const digest = createHash("sha256").update(png).digest("hex");
    const dir = join(dataDir, "media", appId);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, digest), png);
    writeFileSync(join(dir, `${digest}.mime`), "image/png");
    shots[key] = `/v1/apps/${appId}/media/${digest}`;
  }
  const panel = (title, lines, accent) => `<div style="width:100%;height:100vh;box-sizing:border-box;padding:48px;background:linear-gradient(135deg,${accent},#02040a);color:#f7f8fa">
    <div style="font-size:38px;font-weight:700;letter-spacing:-1px">${title}</div>
    <div style="margin-top:28px;padding:28px;border-radius:22px;background:rgba(2,4,10,.55);font:22px/1.8 ui-monospace,Menlo,monospace;color:#cfd6e4">${lines.map((l) => `<div>${l}</div>`).join("")}</div></div>`;
  await shot("briefcaseBanner", "briefcase", 1500, 500, `<div style="width:1500px;height:500px;background:radial-gradient(circle at 15% 20%,#3d7fdc,transparent 55%),radial-gradient(circle at 90% 80%,#7c3aed,transparent 50%),#0b1220;display:flex;align-items:center;padding:0 90px;box-sizing:border-box;color:#fff"><div><div style="font-size:84px;font-weight:700;letter-spacing:-3px">Briefcase</div><div style="font-size:30px;opacity:.8;margin-top:10px">Every file, for every Carbon and Silicon.</div></div></div>`);
  await shot("briefcaseLogo", "briefcase", 256, 256, `<div style="width:256px;height:256px;background:linear-gradient(140deg,#2a6bc6,#163f7a);display:grid;place-items:center"><svg width="150" height="150" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M16 20V4a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"/><rect width="20" height="14" x="2" y="6" rx="2"/></svg></div>`);
  await shot("briefcase1", "briefcase", 1280, 800, panel("Sync a folder in one command", ["$ briefcase sync ./reports", "↑ 14 files uploaded", "✓ reports is in sync"], "#1f5fb8"));
  await shot("briefcase2", "briefcase", 1280, 800, panel("Share with a Silicon", ["$ briefcase share reports si:head_of_growth", "✓ si:head_of_growth can read reports"], "#0e7490"));
  await shot("briefcase3", "briefcase", 1280, 800, panel("Everything as JSON", ["$ briefcase ls --json", '[{"name":"q3.pdf","size":48211}]'], "#6d28d9"));
  await shot("dm1", "dm", 1280, 800, panel("Messages that arrive instantly", ["$ dm send si:nova 'Deploy finished'", "✓ delivered in 84 ms"], "#1f5fb8"));
  await browser.close();
  return shots;
}

const media = await renderMedia();

const apps = [
  app({
    app_id: "briefcase", name: "Briefcase", tags: ["storage", "files", "sync", "productivity"], authors: [people.shubham, people.growth],
    description: "Briefcase keeps your files in one place for you and your Silicons. Sync any folder with one command, share it with a Carbon or a Silicon by their id, and read everything back as JSON. Every change is versioned, so nothing is ever lost, and large files resume where they stopped.\n\nIt runs on all nine targets and signs in with your Silicon Accounts account, so your Silicons use their own identity instead of borrowing yours.",
    production: "3.4.2", development: "3.5.0", installs: 1840, logo: media.briefcaseLogo, banner: media.briefcaseBanner, signer: people.shubham,
    withdrawn: [["production", "3.4.3", "Sync could skip files whose names contain a space. Use 3.4.2 until 3.4.4 is out.", 1]],
    carousel: [
      { url: media.briefcase1, kind: "image", alt: "A terminal running briefcase sync ./reports, which uploads 14 files and reports the folder is in sync." },
      { url: media.briefcase2, kind: "image", alt: "A terminal sharing the reports folder with si:head_of_growth, who can now read it." },
      { url: media.briefcase3, kind: "image", alt: "briefcase ls --json printing the folder as a JSON array." },
    ],
    links: { website: "https://briefcase.example.com", developer_docs: "https://briefcase.example.com/docs", ios: "https://apps.apple.com/app/id000000", android: "https://play.google.com/store/apps/details?id=com.example.briefcase", custom: [{ label: "Changelog", url: "https://briefcase.example.com/changelog", logo: "" }] },
    reviews: [[people.mira, 5, "Syncing a folder for my Silicon took one command. The JSON output is exactly what an agent needs.", 2], [people.nova, 5, "Clear --help, predictable exit codes. I use it every day.", 5], [people.ada, 4, "Fast and simple. I would love selective sync next.", 9], [people.saket, 4, "", 14]],
    created: day(120), updated: day(2),
  }),
  app({
    app_id: "dm", name: "DM", tags: ["messaging", "chat", "notifications"], authors: [people.saket],
    description: "DM is direct messaging between Carbons and Silicons. Send a message from the terminal, watch a conversation live, or let your Silicon answer on its own. Messages are queued durably the moment you send them and delivered in milliseconds, and every account signs in with Silicon Accounts so you always know exactly who you are talking to.",
    production: "0.13.0", installs: 1310, logo: logos.dm, carousel: [{ url: media.dm1, kind: "image", alt: "dm send to si:nova, delivered in 84 milliseconds." }],
    links: { website: "https://dm.example.com" },
    reviews: [[people.nova, 5, "The fastest way for me to reach my carbon.", 1], [people.mira, 4, "Reliable. The live view is great.", 3], [people.ada, 4, "Works well across machines.", 12]],
    created: day(90), updated: day(1),
  }),
  app({
    app_id: "ring", name: "Ring", tags: ["notifications", "messaging", "alerts"], authors: [people.ada, people.nova], targets: TARGETS.filter((t) => !t.startsWith("windows")),
    description: "Ring sends alerts to the right Carbon or Silicon at the right time. Point any script at ring, choose who should hear about it, and it calls, pings or messages them until someone acknowledges. Quiet hours and escalation chains are plain text files you can keep next to your code.",
    production: "1.2.3", development: "1.3.0", installs: 920, logo: logos.ring,
    withdrawn: [["development", "1.3.1", "Escalations fired twice for the same alert.", 2]],
    reviews: [[people.mira, 5, "Escalations finally make sense.", 4], [people.saket, 3, "Good, but I need Windows support.", 20]],
    created: day(60), updated: day(4),
  }),
  app({
    app_id: "remind", name: "Remind", tags: ["productivity", "time", "reminders"], authors: [people.shubham],
    description: "Remind keeps track of what you and your Silicons promised to do. Add a reminder in plain words, like remind me tomorrow at 9 to send the report, and it shows up where you are working, in the terminal or as a notification. Silicons can set reminders for their Carbons too, with their consent.",
    production: "0.2.0", installs: 410, logo: logos.remind,
    reviews: [[people.ada, 4, "Plain words just work.", 6]],
    created: day(30), updated: day(5),
  }),
  app({
    app_id: "notes", name: "Field Notes", tags: ["notes", "writing", "productivity"], authors: [people.mira],
    description: "Field Notes is a fast Markdown notebook for the terminal. Capture a thought in a second, search everything you ever wrote with typos forgiven, and hand any note to a Silicon as clean Markdown or JSON. Notes live as plain files you own, so you can read them with any tool, forever.",
    production: "1.0.0", installs: 12, logo: logos.notes,
    created: day(1), updated: day(1),
  }),
  app({
    app_id: "orbit", name: "Orbit", tags: ["calendar", "time", "scheduling"], authors: [people.growth], targets: ["macos-aarch64", "linux-x86_64"],
    description: "Orbit finds a time that works across every timezone on your team, Carbons and Silicons alike. It reads availability from the calendars you connect, proposes the best slots, and books the meeting once everyone agrees. This is an early development release, so expect rough edges and tell us what breaks.",
    development: "0.4.0", installs: 35, inspected: false,
    created: day(3), updated: day(3),
  }),
  app({
    app_id: "ledgerly", name: "Ledgerly", tags: ["finance", "accounting"], authors: [people.ada],
    description: "Ledgerly is double-entry bookkeeping you can script. Import bank statements, categorise transactions with rules your Silicon can write for you, and export clean reports for your accountant. Every entry is checked so the books always balance, and everything is plain text you can keep in git.",
    production: "2.0.1", installs: 230,
    created: day(45), updated: day(10),
  }),
  app({
    app_id: "silicon-accounts", name: "Silicon Accounts", tags: ["developer-tools", "authentication", "identity"], authors: [people.saket],
    description: "Silicon Accounts gives every Carbon and Silicon one account for every app in the ecosystem. Use the CLI to create your own Silicon account, sign in to apps with short-lived tokens, look after the Silicons you are custodian of, and configure sign-in for the apps you build.",
    production: "0.3.1", installs: 1, links: { website: "https://accounts.teamofsilicons.com", docs: "https://developers.teamofsilicons.com/docs/accounts" },
    created: day(150), updated: day(8),
  }),
  app({
    app_id: "silicon-apps", name: "Silicon Apps", tags: [], authors: [people.saket],
    description: "Silicon Apps is the developer platform, app store and the only updater for every app in the Silicon ecosystem. Search, install and review apps from the terminal, and keep every installed app on its channel up to date automatically.",
    production: "0.2.0", installs: 7, links: { website: "https://apps.teamofsilicons.com" },
    created: day(160), updated: day(8),
  }),
  app({
    app_id: "team-vault", name: "Team Vault", tags: ["security", "secrets"], authors: [people.saket], visibility: "private", access: ["mira"],
    description: "Team Vault keeps the Team's shared secrets and hands each Carbon and Silicon only what they need, for as long as they need it. Every read is logged, every secret can be rotated in one command, and nothing is ever written to disk in plain text.",
    production: "1.1.0", installs: 18, logo: logos.vault,
    created: day(20), updated: day(2),
  }),
  app({
    app_id: "draft-thing", name: "Draft Thing", tags: ["draft"], authors: [people.saket], published: false,
    description: "A draft that must never appear in the store.", installs: 0,
    created: day(1), updated: day(1),
  }),
];

const catalog = { apps: Object.fromEntries(apps.map((a) => [a.app_id, a])), invites: [], reports: [], platforms: {} };
const db = new DatabaseSync(join(dataDir, "apps.sqlite"));
db.exec("PRAGMA busy_timeout = 10000");
db.prepare("UPDATE catalog SET document = ? WHERE id = 1").run(JSON.stringify(catalog));
db.exec("CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,access_token TEXT NOT NULL,refresh_token TEXT,expires_at INTEGER NOT NULL)");
const insert = db.prepare("INSERT OR REPLACE INTO sessions(id, access_token, refresh_token, expires_at) VALUES (?, ?, NULL, ?)");
insert.run("e2e-mira-session", "dev:mira:c:mira", 4102444800);
insert.run("e2e-nova-session", "dev:nova:si:nova", 4102444800);
// Signed out by the e2e suite's sign-out test, so it never ends the sessions other tests use.
insert.run("e2e-ada-session", "dev:ada:c:ada", 4102444800);
db.close();
console.log(JSON.stringify({ seeded: apps.length, data_dir: dataDir, media: Object.keys(media).length, sessions: ["e2e-mira-session", "e2e-nova-session", "e2e-ada-session"] }));
