/** Small formatting helpers shared by pages and JSON-LD. */

export const TARGETS = [
  "macos-aarch64",
  "macos-x86_64",
  "linux-x86_64",
  "linux-aarch64",
  "linux-i686",
  "linux-armv7hf",
  "windows-x86_64",
  "windows-aarch64",
  "windows-i686",
] as const;

const OS_NAMES: Record<string, string> = { macos: "macOS", linux: "Linux", windows: "Windows" };
const ARCH_NAMES: Record<string, string> = {
  "macos-aarch64": "Apple silicon",
  "macos-x86_64": "Intel",
  "linux-x86_64": "x86-64",
  "linux-aarch64": "ARM64",
  "linux-i686": "x86 32-bit",
  "linux-armv7hf": "ARMv7",
  "windows-x86_64": "x64",
  "windows-aarch64": "ARM64",
  "windows-i686": "x86 32-bit",
};

export function osOf(target: string): string {
  return OS_NAMES[target.split("-")[0]] ?? target;
}

export function targetLabel(target: string): string {
  return `${osOf(target)} ${ARCH_NAMES[target] ?? target.split("-").slice(1).join("-")}`;
}

/** Targets grouped by operating system, in a stable order: macOS, Linux, Windows. */
export function targetsByOs(targets: string[]): { os: string; targets: string[] }[] {
  const order = ["macOS", "Linux", "Windows"];
  const groups = new Map<string, string[]>();
  for (const target of [...targets].sort((a, b) => TARGETS.indexOf(a as never) - TARGETS.indexOf(b as never))) {
    const os = osOf(target);
    groups.set(os, [...(groups.get(os) ?? []), target]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (order.indexOf(a) + 1 || 9) - (order.indexOf(b) + 1 || 9))
    .map(([os, list]) => ({ os, targets: list }));
}

export function operatingSystems(targets: string[]): string[] {
  return targetsByOs(targets).map((group) => group.os);
}

export function formatCount(value: number): string {
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${(value / 1000).toFixed(value < 10_000 ? 1 : 0).replace(/\.0$/, "")}k`;
  return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat("en-US").format(value);
}

export function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat("en-US", { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" }).format(date);
}

export function formatRating(rating: number | null | undefined): string {
  return typeof rating === "number" ? rating.toFixed(1) : "";
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${formatNumber(count)} ${count === 1 ? one : many}`;
}

/** Cut at a word boundary, for meta descriptions and cards. */
export function truncate(text: string, max: number): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= max) return clean;
  const cut = clean.slice(0, max - 1);
  const space = cut.lastIndexOf(" ");
  return `${(space > max * 0.6 ? cut.slice(0, space) : cut).replace(/[\s,.;:]+$/, "")}…`;
}

/** Only http(s) and same-origin paths are ever put in an href or src; anything else is dropped. */
export function safeUrl(value: string | undefined | null): string | undefined {
  if (!value) return undefined;
  const trimmed = value.trim();
  if (/^\/(?!\/)/.test(trimmed)) return trimmed;
  try {
    const url = new URL(trimmed);
    return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}

/** Logos may also be data:image URLs (imported from Accounts); those are fine in an <img>, never in a link. */
export function safeImage(value: string | undefined | null): string | undefined {
  if (!value) return undefined;
  const trimmed = value.trim();
  if (/^data:image\/(png|jpe?g|webp|gif|svg\+xml|avif);base64,[A-Za-z0-9+/=\s]+$/i.test(trimmed) && trimmed.length < 2_000_000) return trimmed;
  return safeUrl(trimmed);
}

/** A stable hue per app, for monogram logos and placeholder art. */
export function hueOf(seed: string): number {
  let hash = 0;
  for (const char of seed) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return 205 + (hash % 32);
}

export function initialOf(name: string): string {
  const letter = [...name.trim()].find((char) => /\p{L}|\p{N}/u.test(char));
  return (letter ?? "?").toUpperCase();
}
