export type Account = {
  uuid: string;
  id: string;
  display_name: string;
  verified_emails?: string[];
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
  promoted_from?: string;
};
export type Media = { url: string; kind: "image" | "video"; alt: string };
export type Links = {
  website?: string;
  developer_docs?: string;
  android?: string;
  ios?: string;
  custom?: { label: string; url: string; logo: string }[];
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
  domains?: string[];
  account_ids?: string[];
  links: Links;
  carousel: Media[];
  published: boolean;
  setup_step: number;
  created_at: string;
  updated_at: string;
  authors: Author[];
  targets: string[];
  latest_production: Release | null;
  latest_development: Release | null;
  rating: number | null;
  review_count: number;
  installs: number;
  is_author: boolean;
  is_admin: boolean;
};
export type Package = {
  id: string;
  target: string;
  sha256: string;
  size: number;
  command: string;
  validation: {
    command: string;
    exit_code: number;
    stdout: string;
    stderr: string;
    passed: boolean;
    expected: string;
  }[];
  created_at: string;
};
export type Invite = {
  id: string;
  app_id: string;
  to: string;
  account_uuid?: string;
  status: string;
  created_at: string;
};
export type Review = {
  uuid: string;
  id: string;
  rating: number;
  text: string;
  updated_at: string;
};
export type Readiness = {
  ready: boolean;
  errors: { field: string; message: string }[];
  required_commands: string[];
};
export type Target = {
  target: string;
  population: number | null;
  runner_available: boolean;
};
export const TARGETS = [
  "linux-x86_64",
  "linux-i686",
  "linux-aarch64",
  "linux-armv7hf",
  "windows-x86_64",
  "windows-i686",
  "windows-aarch64",
  "macos-x86_64",
  "macos-aarch64",
] as const;
export const STEPS = [
  "Details",
  "Access",
  "Packages",
  "Links",
  "Media",
  "Account updates",
  "Review and publish",
];
export const EVENTS = [
  "id_change",
  "display_name_change",
  "pfp_change",
  "timezone_change",
  "email_change",
  "phone_change",
  "custodian_change",
  "access_removed",
  "account_deleted",
];
export const DEFAULT_EVENTS = [
  "id_change",
  "display_name_change",
  "pfp_change",
  "access_removed",
  "account_deleted",
];
export const targetLabel = (target: string) =>
  target
    .replace("macos", "macOS")
    .replace("linux", "Linux")
    .replace("windows", "Windows")
    .replace("-aarch64", " · ARM64")
    .replace("-x86_64", " · x64")
    .replace("-i686", " · x86")
    .replace("-armv7hf", " · ARMv7");
