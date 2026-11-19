/**
 * The store's flow (Anil, 2026-11-19: "the complete flow of the apps website"), as data for /playground/sitemap: every
 * page and step a visitor goes through, where it sits in the diagram, and every link between them (read from the
 * pages' code on 2026-11-19). Keep it in step when a page gains or loses a link.
 */

export type Kind = "page" | "step" | "away";

export type FlowNode = {
  id: string;
  label: string;
  /** The address, as a pattern ("/apps/{app_id}"). */
  path: string;
  /** A real address to open (a sample app, category or author), where there is one on this site. */
  open?: string;
  kind: Kind;
  /** What it is, one line. */
  note: string;
  /** Its top-left corner in the diagram (the diagram is 1160 by 500). */
  x: number;
  y: number;
};

export type FlowEdge = { from: string; to: string; label: string };

export const NODE_W = 190;
export const NODE_H = 58;
export const VIEW_W = 1160;
export const VIEW_H = 500;

const SAKET = "240ec600-9f28-4bfc-992e-97635f9c7278";

export const NODES: FlowNode[] = [
  { id: "home", label: "Home", path: "/", open: "/", kind: "page", note: "The banner, Core Apps to Try (Briefcase, DM, Ring) and a card per category.", x: 0, y: 100 },
  { id: "search", label: "Search results", path: "/?q=…", open: "/?q=notes", kind: "page", note: "The home page with ?q=: matching categories, then the matching apps as rows, 24 at a time.", x: 0, y: 300 },
  { id: "all", label: "All Apps", path: "/apps", open: "/apps", kind: "page", note: "Every app by category, the first six of each, with a jump row to each chapter.", x: 240, y: 100 },
  { id: "category", label: "Category", path: "/categories/{id}", open: "/categories/productivity", kind: "page", note: "One category whole: its banner, sort, system and view (cards or list), 24 at a time.", x: 240, y: 300 },
  { id: "app", label: "App page", path: "/apps/{app_id}", open: "/apps/briefcase", kind: "page", note: "Screenshots, About, What’s new, Compatibility and reviews, with install, facts and the developer in a sidebar.", x: 480, y: 200 },
  { id: "author", label: "Developer", path: "/authors/{uuid}", open: `/authors/${SAKET}`, kind: "page", note: "A developer: their facts (apps, installs, systems, who they work with) and every app they publish.", x: 720, y: 100 },
  { id: "signin", label: "Sign in", path: "/sign-in → /v1/auth/login", kind: "step", note: "Not a page: the API starts Silicon Accounts sign-in (PKCE), then sends you back where you were.", x: 720, y: 250 },
  { id: "account", label: "Account menu", path: "sidebar, signed in", kind: "step", note: "The sidebar’s account button: who you are, Manage your account, Sign Out. (/settings, retired 2026-11-19, redirects home.)", x: 720, y: 400 },
  { id: "report", label: "Report a problem", path: "sidebar foot → dialog", kind: "step", note: "A dialog from the sidebar’s foot: what happened and a pull request, sent to the Team; a notice on the page after (?report=).", x: 480, y: 400 },
  { id: "developers", label: "Developer portal", path: "↗ developers site", kind: "away", note: "Publishing and managing apps (Manage App lands on the app’s Publishing page).", x: 960, y: 60 },
  { id: "docs", label: "Docs", path: "↗ developers/docs/apps", kind: "away", note: "Make a Silicon App: the CLI, packages, releases and signing.", x: 960, y: 160 },
  { id: "accounts", label: "Silicon Accounts", path: "↗ accounts site", kind: "away", note: "Where you sign in and manage your account; it hands you back to the store.", x: 960, y: 260 },
  { id: "signout", label: "Sign out", path: "POST /sign-out", kind: "step", note: "Not a page: ends the session (and the Accounts token), then back to the page you were on.", x: 960, y: 400 },
];

export const EDGES: FlowEdge[] = [
  { from: "home", to: "all", label: "See all apps" },
  { from: "home", to: "category", label: "A category card’s Explore apps" },
  { from: "home", to: "app", label: "Core Apps to Try" },
  { from: "search", to: "app", label: "A result" },
  { from: "search", to: "search", label: "More results" },
  { from: "all", to: "category", label: "A chapter’s See all" },
  { from: "all", to: "app", label: "An app in a chapter" },
  { from: "category", to: "app", label: "An app" },
  { from: "category", to: "category", label: "Sort, system, view, Show More" },
  { from: "app", to: "author", label: "The developer card; More by’s See all" },
  { from: "app", to: "app", label: "More by the developer" },
  { from: "app", to: "category", label: "The category chip" },
  { from: "app", to: "search", label: "A tag" },
  { from: "app", to: "signin", label: "Sign in to write a review" },
  { from: "app", to: "developers", label: "Manage App (its authors)" },
  { from: "app", to: "docs", label: "Get the CLI; how signing works" },
  { from: "author", to: "app", label: "One of their apps" },
  { from: "author", to: "author", label: "Works with" },
  { from: "account", to: "signout", label: "Sign Out" },
  { from: "account", to: "accounts", label: "Manage your account" },
  { from: "signin", to: "accounts", label: "Sign in with Silicon Accounts" },
  { from: "accounts", to: "signin", label: "Back through /v1/auth/callback, to the page you were on" },
  { from: "signout", to: "app", label: "Back to the page you were on" },
];

/** On every page, the sidebar (components/shell/shell.tsx). */
export const SIDEBAR: { label: string; to: string }[] = [
  { label: "Search apps", to: "search" },
  { label: "Home", to: "home" },
  { label: "All Apps", to: "all" },
  { label: "Each category", to: "category" },
  { label: "Account menu (signed in)", to: "account" },
  { label: "Sign In (signed out)", to: "signin" },
  { label: "Developers, Docs", to: "developers" },
  { label: "Report a problem", to: "report" },
];

/** Old addresses that still work (proxy.ts), each a permanent redirect. */
export const REDIRECTS: { from: string; to: string }[] = [
  { from: "/search?q=…", to: "/?q=…" },
  { from: "/apps?q=…", to: "/?q=…" },
  { from: "/store, /store?…", to: "/ (with the search)" },
  { from: "/store/{app_id}", to: "/apps/{app_id}" },
  { from: "/store/apps, /apps/apps…", to: "/apps/silicon-apps" },
  { from: "/developer/…", to: "the developer portal (an app’s tabs to its pages there)" },
  { from: "/docs/…", to: "the developer portal’s /docs/apps/…" },
  { from: "/settings", to: "/ (retired; its parts are in the sidebar)" },
];

/** What the site serves for machines rather than people. */
export const MACHINES: { path: string; note: string }[] = [
  { path: "/sitemap.xml", note: "Every public page, for search engines" },
  { path: "/robots.txt", note: "What crawlers may read" },
  { path: "/llms.txt, /llms-full.txt", note: "The store explained for agents" },
  { path: "/manifest.webmanifest", note: "Install as an app" },
  { path: "/.well-known/security.txt", note: "Where to report a vulnerability" },
  { path: "/og.png, /apps/{app_id}/og.png", note: "Share pictures" },
  { path: "/v1/…", note: "The Apps API, passed through" },
];
