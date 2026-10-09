/**
 * Search and answer-engine metadata for the store, as on the developer site (developer/lib/seo.tsx): one helper that
 * gives every page its full set (title, description, canonical, Open Graph, Twitter), and the Schema.org JSON-LD the
 * pages embed. Next merges metadata objects shallowly, so each page passes everything here rather than relying on a
 * parent's openGraph.
 */
import type { Metadata } from "next";
import { operatingSystems, safeImage, truncate } from "./format";
import { CANONICAL_ORIGIN, LINKS, OG_IMAGE, ORGANIZATION, SITE_DESCRIPTION, SITE_NAME, installCommand } from "./site";
import type { App, AuthorProfile, Review } from "./types";

export interface PageMeta {
  /** The page's own title; " · Silicon Apps" follows it unless `absoluteTitle`. */
  title: string;
  description: string;
  /** The page's path on the canonical origin: "/", "/apps/briefcase". */
  path: string;
  type?: "website" | "article" | "profile";
  absoluteTitle?: boolean;
  /** The social image: a path on this site ("/apps/ring/og.png") or the shared one. */
  image?: { url: string; alt: string };
  index?: boolean;
}

export const absolute = (path: string) => (path === "/" ? `${CANONICAL_ORIGIN}/` : `${CANONICAL_ORIGIN}${path}`);

export function pageMetadata({ title, description, path, type = "website", absoluteTitle = false, image, index = true }: PageMeta): Metadata {
  const url = absolute(path);
  const fullTitle = absoluteTitle ? title : `${title} · ${SITE_NAME}`;
  const text = truncate(description, 200);
  const picture = image ?? { url: OG_IMAGE.url, alt: OG_IMAGE.alt };
  const pictureUrl = picture.url.startsWith("http") ? picture.url : `${CANONICAL_ORIGIN}${picture.url}`;
  return {
    title: { absolute: fullTitle },
    description: text,
    alternates: { canonical: url, types: { "text/plain": "/llms.txt" } },
    robots: index ? { index: true, follow: true } : { index: false, follow: true },
    openGraph: {
      type: type === "profile" ? "profile" : type,
      url,
      siteName: SITE_NAME,
      title: fullTitle,
      description: text,
      locale: "en_US",
      images: [{ url: pictureUrl, width: OG_IMAGE.width, height: OG_IMAGE.height, alt: picture.alt }],
    },
    twitter: { card: "summary_large_image", title: fullTitle, description: text, images: [pictureUrl] },
  };
}

/* ------------------------------------------------------------------------------------------------------------------ */
/* JSON-LD                                                                                                             */
/* ------------------------------------------------------------------------------------------------------------------ */

export type Json = string | number | boolean | null | Json[] | { [key: string]: Json | undefined };

const ORGANIZATION_ID = `${ORGANIZATION.url}/#organization`;
const WEBSITE_ID = `${CANONICAL_ORIGIN}/#website`;

export function organizationLd(): Json {
  return {
    "@type": "Organization",
    "@id": ORGANIZATION_ID,
    name: ORGANIZATION.name,
    url: ORGANIZATION.url,
    logo: `${CANONICAL_ORIGIN}/icon-512.png`,
    email: ORGANIZATION.email,
    sameAs: [LINKS.appsGithub, LINKS.accountsGithub],
  };
}

export function websiteLd(): Json {
  return {
    "@type": "WebSite",
    "@id": WEBSITE_ID,
    name: SITE_NAME,
    url: `${CANONICAL_ORIGIN}/`,
    description: SITE_DESCRIPTION,
    inLanguage: "en",
    publisher: { "@id": ORGANIZATION_ID },
    potentialAction: {
      "@type": "SearchAction",
      target: { "@type": "EntryPoint", urlTemplate: `${CANONICAL_ORIGIN}/search?q={search_term_string}` },
      "query-input": "required name=search_term_string",
    },
  };
}

export interface Crumb {
  name: string;
  path: string;
}

export function breadcrumbLd(crumbs: Crumb[]): Json {
  return {
    "@type": "BreadcrumbList",
    itemListElement: crumbs.map((crumb, index) => ({ "@type": "ListItem", position: index + 1, name: crumb.name, item: absolute(crumb.path) })),
  };
}

export const appPath = (appId: string) => `/apps/${encodeURIComponent(appId)}`;
export const authorPath = (uuid: string) => `/authors/${encodeURIComponent(uuid)}`;

const agentType = (id: string) => (id.startsWith("si:") ? "Organization" : "Person");

/** An app as a SoftwareApplication: what it is, where it runs, what it costs (nothing), its rating and its authors. */
export function softwareApplicationLd(app: App, reviews: Review[] = []): Json {
  const release = app.latest_production ?? app.latest_development;
  const logo = safeImage(app.logo);
  const url = absolute(appPath(app.app_id));
  const node: { [key: string]: Json | undefined } = {
    "@type": "SoftwareApplication",
    "@id": `${url}#app`,
    name: app.name,
    identifier: app.app_id,
    url,
    description: app.description || undefined,
    applicationCategory: "DeveloperApplication",
    applicationSubCategory: "Command line app",
    operatingSystem: operatingSystems(app.targets).join(", ") || undefined,
    softwareVersion: release?.version,
    datePublished: app.created_at,
    dateModified: app.updated_at,
    keywords: app.tags.length ? app.tags.join(", ") : undefined,
    image: logo && !logo.startsWith("data:") ? new URL(logo, CANONICAL_ORIGIN).toString() : absolute(`${appPath(app.app_id)}/og.png`),
    screenshot: (app.carousel ?? []).filter(item => item.kind === "image" && safeImage(item.url) && !item.url.startsWith("data:")).slice(0, 5).map(item => new URL(item.url, CANONICAL_ORIGIN).toString()),
    installUrl: `${url}#install`,
    downloadUrl: `${url}#install`,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD", availability: "https://schema.org/InStock", url },
    author: app.authors.map(author => ({ "@type": agentType(author.id), name: author.display_name || author.id, identifier: author.id, url: absolute(authorPath(author.uuid)) })),
    publisher: { "@id": ORGANIZATION_ID },
    isPartOf: { "@id": WEBSITE_ID },
    potentialAction: { "@type": "InstallAction", name: installCommand(app.app_id), target: `${url}#install` },
  };
  if (!(node.screenshot as Json[]).length) delete node.screenshot;
  if (app.review_count > 0 && app.rating !== null) {
    node.aggregateRating = { "@type": "AggregateRating", ratingValue: Number(app.rating.toFixed(2)), ratingCount: app.review_count, reviewCount: app.review_count, bestRating: 5, worstRating: 1 };
    node.review = reviews.slice(0, 10).map(review => ({
      "@type": "Review",
      author: { "@type": agentType(review.id), name: review.id },
      datePublished: review.updated_at,
      reviewBody: review.text || undefined,
      reviewRating: { "@type": "Rating", ratingValue: review.rating, bestRating: 5, worstRating: 1 },
    }));
  }
  return node;
}

export function profilePageLd(author: AuthorProfile, path: string): Json {
  return {
    "@type": "ProfilePage",
    "@id": `${absolute(path)}#page`,
    url: absolute(path),
    name: `${author.display_name || author.id} on ${SITE_NAME}`,
    isPartOf: { "@id": WEBSITE_ID },
    mainEntity: { "@type": agentType(author.id), name: author.display_name || author.id, identifier: author.id, url: absolute(path) },
  };
}

export function itemListLd(name: string, apps: App[]): Json {
  return {
    "@type": "ItemList",
    name,
    numberOfItems: apps.length,
    itemListElement: apps.map((app, index) => ({ "@type": "ListItem", position: index + 1, url: absolute(appPath(app.app_id)), name: app.name })),
  };
}

export function faqLd(entries: Array<{ question: string; answer: string }>): Json {
  return {
    "@type": "FAQPage",
    mainEntity: entries.map(entry => ({ "@type": "Question", name: entry.question, acceptedAnswer: { "@type": "Answer", text: entry.answer } })),
  };
}

/** One <script type="application/ld+json"> with a @graph. `<` is escaped so no string can close the script. */
export function JsonLd({ graph, nonce }: { graph: Json[]; nonce?: string }) {
  const body = JSON.stringify({ "@context": "https://schema.org", "@graph": graph }).replace(/</g, "\\u003c").replace(/\u2028/g, "\\u2028").replace(/\u2029/g, "\\u2029");
  return <script type="application/ld+json" nonce={nonce} dangerouslySetInnerHTML={{ __html: body }} />;
}
