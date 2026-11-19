/**
 * The type lab's values (Anil, 2026-11-09; the font lab of 2026-11-07, grown), development only. The site's typography is
 * Silicon UI's tokens (styles/tokens.css): two faces, nine size steps, tracking and line height for display and body,
 * and our three weights. Every Silicon UI component and every type role reads them, so the lab sets them on :root and
 * the whole interface retypes live. No colours: those stay in tokens.css. The values are kept in this browser
 * (`store.dev.type-lab`) and the dev panel keeps their stylesheet on every page until Reset. A value left unset
 * leaves the page's own. Copy CSS gives a :root block to paste into styles/tokens.css.
 *
 * Copied from the account site (silicon-accounts web, lib/type-lab.ts, 2026-11-14), with the same faces: app/fonts.ts
 * loads them for the lab in development only (the store itself uses the system font).
 */
export const LAB_FONTS = [
  { value: "page", label: "The page's own", css: "" },
  { value: "hanken", label: "Hanken Grotesk", css: "var(--font-hanken-grotesk), sans-serif" },
  { value: "geist", label: "Geist", css: "var(--font-geist), sans-serif" },
  { value: "inter", label: "Inter", css: "var(--font-inter), sans-serif" },
  { value: "system", label: "System font", css: "system-ui, -apple-system, BlinkMacSystemFont, sans-serif" },
  { value: "work-sans", label: "Work Sans", css: "var(--font-work-sans), sans-serif" },
  { value: "tex-gyre-heros", label: "TeX Gyre Heros", css: "var(--font-tex-gyre-heros), sans-serif" },
  { value: "liberation-sans", label: "Liberation Sans", css: "var(--font-liberation-sans), sans-serif" },
  { value: "cooper-hewitt", label: "Cooper Hewitt", css: "var(--font-cooper-hewitt), sans-serif" },
  { value: "bdo", label: "BDO Grotesk", css: "var(--font-bdo-grotesk), sans-serif" },
];

/** Silicon UI's size steps, smallest first. */
export const STEPS = ["xs", "sm", "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl"] as const;
export type Step = (typeof STEPS)[number];

export interface TypeLabValues {
  textFont?: string;
  titleFont?: string;
  /** Titles use the text font. */
  sameFont?: boolean;
  /** Inter's and Hanken's alternates: cv01 (a plainer 1) and ss03 (rounder punctuation). */
  features?: boolean;
  /** Each step's size in px. */
  sizes?: Partial<Record<Step, number>>;
  /** Tracking in em. */
  trackingDisplay?: number;
  trackingBody?: number;
  /** Line height as a ratio. */
  leadingDisplay?: number;
  leadingBody?: number;
  weightTitle?: number;
  weightMedium?: number;
  weightRegular?: number;
}

/** Starting points; each fills only type values, never colours. */
export const PRESETS: { value: string; label: string; values: TypeLabValues }[] = [
  { value: "silicon-ui", label: "Silicon UI (the store)", values: {} },
  { value: "silicon-ui-geist", label: "Silicon UI with Geist titles", values: { titleFont: "geist" } },
];

const KEY = "store.dev.type-lab";
/** The font lab's key before 2026-11-09; its values don't map onto the tokens, so they are dropped. */
const OLD_KEY = "store.dev.font-lab";
const listeners = new Set<() => void>();

/** For useSyncExternalStore: the lab and the dev panel re-read the values when they change. */
export const subscribeTypeLab = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

/** The stored values as their JSON text: a stable snapshot for useSyncExternalStore. */
export function readTypeLabText(): string {
  try {
    window.localStorage.removeItem(OLD_KEY);
    return window.localStorage.getItem(KEY) ?? "{}";
  } catch {
    return "{}";
  }
}

export function parseTypeLab(text: string): TypeLabValues {
  try {
    const value = JSON.parse(text) as unknown;
    return value && typeof value === "object" ? (value as TypeLabValues) : {};
  } catch {
    return {};
  }
}

export function writeTypeLab(values: TypeLabValues) {
  try {
    if (Object.keys(values).length) window.localStorage.setItem(KEY, JSON.stringify(values));
    else window.localStorage.removeItem(KEY);
  } catch {
    // Storage blocked: nothing is kept.
  }
  listeners.forEach(listener => listener());
}

export const fontCss = (value?: string) => LAB_FONTS.find(font => font.value === value)?.css ?? "";

const rem = (px: number) => `${Number((px / 16).toFixed(4))}rem`;

/** The stylesheet for a set of values, also what Copy CSS gives. Empty when nothing is changed. */
export function typeLabCss(values: TypeLabValues): string {
  const text = fontCss(values.textFont);
  const title = values.sameFont ? text || "var(--font-body)" : fontCss(values.titleFont);
  const lines = [
    text && `--font-body: ${text}`,
    title && `--font-display: ${title}`,
    ...STEPS.map(step => values.sizes?.[step] !== undefined && `--text-${step}: ${rem(values.sizes[step]!)}`),
    values.trackingDisplay !== undefined && `--tracking-display: ${values.trackingDisplay}em`,
    values.trackingBody !== undefined && `--tracking-body: ${values.trackingBody}em`,
    values.leadingDisplay !== undefined && `--leading-display: ${values.leadingDisplay}`,
    values.leadingBody !== undefined && `--leading-body: ${values.leadingBody}`,
    values.weightTitle !== undefined && `--weight-title: ${values.weightTitle}`,
    values.weightMedium !== undefined && `--weight-medium: ${values.weightMedium}`,
    values.weightRegular !== undefined && `--weight-regular: ${values.weightRegular}`,
  ].filter(Boolean);
  return [
    lines.length ? `:root {\n${lines.map(line => `  ${line} !important;`).join("\n")}\n}` : "",
    values.features ? `body {\n  font-feature-settings: "cv01", "ss03";\n}` : "",
  ].filter(Boolean).join("\n");
}

/** What the page's tokens are without the lab, read from :root. Sizes in px. */
export interface TokenDefaults {
  sizes: Record<Step, number>;
  trackingDisplay: number;
  trackingBody: number;
  leadingDisplay: number;
  leadingBody: number;
  weightTitle: number;
  weightMedium: number;
  weightRegular: number;
}

export function readTokenDefaults(sheetName: string): TokenDefaults {
  const sheet = document.head.querySelector<HTMLStyleElement>(`style[data-dev-style="${sheetName}"]`)?.sheet;
  const wasDisabled = sheet?.disabled;
  if (sheet) sheet.disabled = true;
  try {
    const root = getComputedStyle(document.documentElement);
    const number = (name: string, fallback: number) => {
      const value = parseFloat(root.getPropertyValue(name));
      return Number.isNaN(value) ? fallback : value;
    };
    const rootPx = parseFloat(root.fontSize) || 16;
    const px = (name: string, fallback: number) => {
      const raw = root.getPropertyValue(name).trim();
      const value = parseFloat(raw);
      if (Number.isNaN(value)) return fallback;
      return Math.round(raw.endsWith("px") ? value : value * rootPx);
    };
    const fallbackSizes = [12, 14, 16, 18, 22, 28, 36, 52, 72];
    return {
      sizes: Object.fromEntries(STEPS.map((step, index) => [step, px(`--text-${step}`, fallbackSizes[index]!)])) as Record<Step, number>,
      trackingDisplay: number("--tracking-display", -0.03),
      trackingBody: number("--tracking-body", -0.01),
      leadingDisplay: number("--leading-display", 1.1),
      leadingBody: number("--leading-body", 1.4),
      weightTitle: number("--weight-title", 600),
      weightMedium: number("--weight-medium", 500),
      weightRegular: number("--weight-regular", 400),
    };
  } finally {
    if (sheet) sheet.disabled = wasDisabled ?? false;
  }
}
