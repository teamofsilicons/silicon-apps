/**
 * The root layout of apps.teamofsilicons.com: <html> and <body>, the shared faces and tokens over Arc's foundation (the
 * same files as the developer site), the no-flash theme boot script and the WebMCP tools (both inline, with the
 * request's CSP nonce), the header and footer, the one behaviour island, and the Organization and WebSite JSON-LD
 * every page carries. Every page renders per request: the nonce and the visitor's session change each time.
 */
import "@/components/arc/foundation.css";
import "@/styles/fonts.css";
import "@/styles/tokens.css";
import "@/styles/squircle.css";
import "@/styles/base.css";
import type { Metadata, Viewport } from "next";
import { headers } from "next/headers";
import type { ReactNode } from "react";
import { Enhancer } from "@/components/site/enhancer";
import { SiteFooter } from "@/components/site/site-footer";
import { SiteHeader } from "@/components/site/site-header";
import { JsonLd, organizationLd, websiteLd } from "@/lib/seo";
import { getAccount } from "@/lib/session";
import { CANONICAL_ORIGIN, OG_IMAGE, SITE_DESCRIPTION, SITE_NAME } from "@/lib/site";
import { THEME_BOOT_SCRIPT } from "@/lib/theme";
import { WEBMCP_SCRIPT } from "@/lib/webmcp";

export const metadata: Metadata = {
  metadataBase: new URL(CANONICAL_ORIGIN),
  title: { default: `${SITE_NAME}: apps for Carbons and Silicons`, template: `%s · ${SITE_NAME}` },
  description: SITE_DESCRIPTION,
  applicationName: SITE_NAME,
  referrer: "strict-origin-when-cross-origin",
  manifest: "/manifest.webmanifest",
  icons: {
    icon: [{ url: "/favicon.ico", sizes: "32x32" }, { url: "/icon.svg", type: "image/svg+xml" }],
    apple: [{ url: "/apple-touch-icon.png", sizes: "180x180" }],
  },
  openGraph: { type: "website", siteName: SITE_NAME, locale: "en_US", images: [{ ...OG_IMAGE, url: `${CANONICAL_ORIGIN}${OG_IMAGE.url}` }] },
  twitter: { card: "summary_large_image" },
  formatDetection: { telephone: false, email: false, address: false },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#F7F8FA" },
    { media: "(prefers-color-scheme: dark)", color: "#02040A" },
  ],
};

/** This page's path and query, set by proxy.ts (for the current link and for coming back after signing in). */
async function currentPath(): Promise<string> {
  const value = (await headers()).get("x-store-path") || "/";
  return value.startsWith("/") && !value.startsWith("//") ? value : "/";
}

export default async function RootLayout({ children }: { children: ReactNode }) {
  const [incoming, account, path] = await Promise.all([headers(), getAccount(), currentPath()]);
  const nonce = incoming.get("x-nonce") ?? undefined;
  return (
    <html lang="en" data-surface="site" suppressHydrationWarning>
      <head>
        <script nonce={nonce} dangerouslySetInnerHTML={{ __html: THEME_BOOT_SCRIPT }} />
        {/* Page titles are set in DemiBold: the one weight worth fetching before first paint. */}
        <link rel="preload" href="/fonts/bdo-grotesk/BDOGrotesk-DemiBold.woff2" as="font" type="font/woff2" crossOrigin="anonymous" />
      </head>
      <body>
        <SiteHeader path={path} account={account} />
        <main id="main" tabIndex={-1}>
          {children}
        </main>
        <SiteFooter />
        <Enhancer />
        <JsonLd nonce={nonce} graph={[organizationLd(), websiteLd()]} />
        <script nonce={nonce} dangerouslySetInnerHTML={{ __html: WEBMCP_SCRIPT }} />
      </body>
    </html>
  );
}
