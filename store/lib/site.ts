/**
 * Who this site is and where everything else lives: the names, addresses and words the pages, the metadata, the agent
 * files (robots.txt, sitemap.xml, llms.txt) and the MCP server share. Imports nothing, so server and client code can
 * both use it. Runtime addresses that change per stack (the Apps API, this store's own origin) are in lib/config.ts.
 */

/** The store's public origin: canonical links, Open Graph, JSON-LD and the sitemap always name it. */
export const CANONICAL_ORIGIN = "https://apps.teamofsilicons.com";

export const SITE_NAME = "Silicon Apps";
/** One line about the site: the home page's description and the default for every page. */
export const SITE_DESCRIPTION =
  "Silicon Apps is the app store of the Silicon ecosystem. Every app is a command line app first, made for Carbons and Silicons alike, installs with one command and keeps itself up to date.";

export const ORGANIZATION = {
  name: "Team of Silicons",
  url: "https://teamofsilicons.com",
  email: "lords@teamofsilicons.com",
} as const;

export const LINKS = {
  teamOfSilicons: "https://teamofsilicons.com",
  developers: "https://developers.teamofsilicons.com",
  /** "Make an app": the Silicon Apps docs on the developer site. */
  appsDocs: "https://developers.teamofsilicons.com/docs/apps",
  /** "Add sign-in": the Silicon Accounts docs on the developer site. */
  accountsDocs: "https://developers.teamofsilicons.com/docs/accounts",
  installDocs: "https://developers.teamofsilicons.com/docs/apps/start/install",
  accounts: "https://accounts.teamofsilicons.com",
  accountsLlms: "https://accounts.teamofsilicons.com/llms.txt",
  developersLlms: "https://developers.teamofsilicons.com/llms.txt",
  /** Whether Silicon Apps and Silicon Accounts are up, on the developer site. */
  status: "https://developers.teamofsilicons.com/status",
  /** Silicon Apps and Silicon Accounts are both open source under the MIT license. */
  appsGithub: "https://github.com/teamofsilicons/silicon-apps",
  accountsGithub: "https://github.com/teamofsilicons/silicon-accounts",
} as const;

/** An app's page on the developer site, where its authors manage it. */
export const manageAppUrl = (appId: string) => `${LINKS.developers}/apps/${encodeURIComponent(appId)}/publishing`;

/** The shared social image (1200 by 630, public/og.png). */
export const OG_IMAGE = { url: "/og.png", width: 1200, height: 630, alt: "Silicon Apps: apps for Carbons and Silicons" } as const;

/** The MCP protocol versions /mcp speaks, newest first. */
export const MCP_PROTOCOL_VERSIONS = ["2025-06-18", "2025-03-26", "2024-11-05"] as const;

/** Rate limits of this site's own endpoints, per client address. The Apps API has its own (see /openapi.json). */
export const RATE_LIMITS = {
  mcp: { limit: 60, windowSeconds: 60 },
} as const;

/** The command that installs an app, as every page and tool shows it. */
export const installCommand = (appId: string) => `silicon-apps install ${appId}`;

export const INSTALL_UNIX = `curl -fsSL https://apps.teamofsilicons.com/install.sh -o install-apps.sh &&
bash install-apps.sh --server https://apps.teamofsilicons.com &&
export PATH="\${SILICON_HOME:-$HOME}/.apps/bin:$PATH"`;

export const INSTALL_WINDOWS = `Invoke-WebRequest -UseBasicParsing https://apps.teamofsilicons.com/install.ps1 -OutFile install-apps.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\\install-apps.ps1 -Server https://apps.teamofsilicons.com`;
