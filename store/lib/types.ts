/** Shapes returned by the Apps API (see API_CONTRACT.md in the repository root). */

export type Account = {
  uuid: string;
  id: string;
  display_name: string;
  pfp_url?: string | null;
};

export type Author = Account & { joined_at: string };

export type Release = {
  id: string;
  app_id: string;
  channel: "production" | "development";
  version: string;
  package_ids: string[];
  notes: string;
  created_at: string;
  promoted_from?: string | null;
  /** The Apps API's signature over each package, by package id. */
  signatures?: Record<string, { key_id: string; algorithm: string; signed_at: string }>;
  /** Every package also carries its author's own signature. */
  signed_by_author?: boolean;
  /** Set when an author withdrew it; a withdrawn release is never served. */
  withdrawn?: Withdrawal | null;
};

export type Withdrawal = { at: string; by_uuid: string; by_id: string; reason: string };

/** A withdrawn release as the App object lists it (newest version first). */
export type WithdrawnRelease = {
  release_id: string;
  version: string;
  channel: "production" | "development";
  reason: string;
  withdrawn_at: string;
  withdrawn_by: string;
};

export type Media = { url: string; kind: "image" | "video"; alt?: string };

export type CustomLink = { label: string; url: string; logo?: string };

export type Links = {
  website?: string;
  developer_docs?: string;
  docs?: string;
  android?: string;
  ios?: string;
  custom?: CustomLink[];
};

export type App = {
  app_id: string;
  name: string;
  description: string;
  logo: string;
  logo_alt?: string;
  banner: string;
  banner_alt?: string;
  tags: string[];
  visibility: "public" | "private";
  links: Links | null;
  interface_api_documentation_url?: string | null;
  interface_use_cases?: { name: string; files: { name: string; content: string }[] }[] | null;
  interface_license?: { kind: "preset"; preset: "free" | "attribution" | "attribution_unless_paid" | "permission_required" } | { kind: "link"; url: string } | { kind: "file"; name: string; content: string } | null;
  carousel: Media[];
  published: boolean;
  created_at: string;
  updated_at: string;
  authors: Author[];
  targets: string[];
  /** The authors' public reason for each required target (linux-x86_64, windows-x86_64, macos-aarch64) the current
   * release leaves out. */
  unsupported_targets?: Record<string, string>;
  latest_production: Release | null;
  latest_development: Release | null;
  /** Every package of the current release (production, else development) carries the Apps API's signature. */
  signed?: boolean;
  /** Every package of the current release also carries its author's own signature. */
  signed_by_author?: boolean;
  /** Who signed the current release's packages as author, by current id (c:… or si:…). */
  signed_by?: string[];
  withdrawn_releases?: WithdrawnRelease[];
  rating: number | null;
  review_count: number;
  installs: number;
  is_author: boolean;
  is_admin: boolean;
};

export type AppList = { items: App[]; total: number };

export type Review = {
  uuid: string;
  id: string;
  rating: number;
  text: string;
  updated_at: string;
};

export type Reviews = { items: Review[]; rating: number | null; count: number };

export type AuthorProfile = Account & { items: App[]; total: number };

export type Session = { authenticated: boolean; account: Account | null };

export type ApiErrorBody = { error?: { code?: string; message?: string; hint?: string; details?: unknown } };
