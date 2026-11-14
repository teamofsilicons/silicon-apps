/**
 * The root layout of apps.teamofsilicons.com: <html> and <body>, Silicon UI's styles under the account site's tokens
 * (styles/tokens.css), the boot script (next/script beforeInteractive, with the request's CSP nonce; the store is light
 * only), the shell (top
 * bar, section list and the page box, components/shell), and the Organization and WebSite JSON-LD every page carries. Every page renders per request: the nonce and the visitor's session change each time.
 */
import "@teamofsilicons/silicon-ui/styles.css";
import "@/styles/tokens.css";
import "@/styles/squircle.css";
import "@/styles/base.css";
import type { Metadata, Viewport } from "next";
import { headers } from "next/headers";
import Script from "next/script";
import type { ReactNode } from "react";
import { DevTools } from "@/components/dev/dev-tools";
import { labFontVariables } from "./fonts";
import { Shell } from "@/components/shell/shell";
import { JsonLd, organizationLd, websiteLd } from "@/lib/seo";
import { getAccount } from "@/lib/session";
import { CANONICAL_ORIGIN, OG_IMAGE, SITE_DESCRIPTION, SITE_NAME } from "@/lib/site";
import { BOOT_SCRIPT } from "@/lib/boot";

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
  themeColor: "#FCFCFC",
  colorScheme: "light",
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
    <html lang="en" data-surface="site" className={process.env.NODE_ENV === "development" ? labFontVariables : undefined} suppressHydrationWarning>
      <body>
        {/* Before anything paints or hydrates, through Next's own Script (beforeInteractive), with the request's nonce:
            an inline <script> in the layout's tree makes React 19 warn in development, since the client re-renders it. */}
        <Script id="store-boot" strategy="beforeInteractive" nonce={nonce} dangerouslySetInnerHTML={{ __html: BOOT_SCRIPT }} />
        <Shell path={path} account={account}>{children}</Shell>
        {/* The account site's dev tools (type lab, text edits, inspectors, density), in `next dev` only. */}
        {process.env.NODE_ENV === "development" ? <DevTools /> : null}
        <JsonLd nonce={nonce} graph={[organizationLd(), websiteLd()]} />
      </body>
    </html>
  );
}
