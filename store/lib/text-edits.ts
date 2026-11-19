/**
 * Edit text (Anil, 2026-11-09), development only: changes to one kind of text, made by clicking it with the type
 * inspector on (components/foundation/dev/type-inspector.tsx). Each edit belongs to the CSS rule that styles that text
 * (its selector), so it reaches every text that rule styles, on every page. Kept in this browser
 * (`store.dev.text-edits`) as selector → values; the dev panel keeps their stylesheet on every page until cleared.
 * An edit can also be for one text only ("Just this one"): then its selector is that element's path in the page (from
 * the nearest id or <main>), which holds while the page's structure does. The dev tools themselves
 * ([data-type-inspect-exempt]) are left out. Copy CSS gives the edits grouped by source, ready
 * to move into code; Silicon UI's own rules can't be restyled (AGENTS.md), so those are marked as needing a token or a
 * request to Silicon UI.
 */
import { fontCss } from "./type-lab";

export const ROLES = ["display", "h1", "h2", "h3", "body", "label", "small", "caption", "hint"] as const;

/** The text colour tokens offered first, by plain name; any other colour is a custom one. */
export const TEXT_COLOURS = [
  { token: "--foreground", name: "Primary" },
  { token: "--text-soft", name: "Soft" },
  { token: "--text-secondary", name: "Secondary" },
  { token: "--text-muted", name: "Muted" },
  { token: "--text-faint", name: "Faint" },
  { token: "--accent-ink", name: "Link" },
  { token: "--accent", name: "Accent" },
  { token: "--primary-foreground", name: "On primary" },
  { token: "--success", name: "Success" },
  { token: "--warning", name: "Warning" },
  { token: "--danger", name: "Danger" },
];

/**
 * platform.openai.com's text colours (its light theme, read from its Home page on 2026-11-09), to try beside ours.
 * Kept as plain colours, since they aren't our tokens; the ones without a token name there are named by use.
 */
export const OPENAI_COLOURS = [
  { value: "#0D0D0D", name: "Primary" },
  { value: "#181818", name: "Section title" },
  { value: "#282828", name: "Text (nav, page title)" },
  { value: "#414141", name: "Label" },
  { value: "#5D5D5D", name: "Secondary" },
  { value: "#8F8F8F", name: "Tertiary" },
  { value: "#6B3AB4", name: "New badge" },
];
/** A colour from a list above, rather than one picked with the custom picker. */
export const isListedColour = (colour: string) => colour.startsWith("--") || OPENAI_COLOURS.some(item => item.value === colour);

export interface TextEdit {
  /** Where the rule comes from, for the editor's header and Copy CSS, e.g. "silicons.module.css .sectionTitle". */
  source: string;
  /** The rule is one of Silicon UI's own. */
  component?: boolean;
  /** For one text only: the start of its words, to name it. */
  only?: string;
  role?: (typeof ROLES)[number];
  font?: string;
  /** A size step (xs to 5xl) or px. */
  size?: string | number;
  weight?: number;
  /** In em. */
  tracking?: number;
  /** As a ratio. */
  line?: number;
  /** A token name (--foreground) or a colour. */
  colour?: string;
}

export type TextEdits = Record<string, TextEdit>;

const KEY = "store.dev.text-edits";
const listeners = new Set<() => void>();

export const subscribeTextEdits = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export function readTextEditsText(): string {
  try {
    return window.localStorage.getItem(KEY) ?? "{}";
  } catch {
    return "{}";
  }
}

export function parseTextEdits(text: string): TextEdits {
  try {
    const value = JSON.parse(text) as unknown;
    return value && typeof value === "object" ? (value as TextEdits) : {};
  } catch {
    return {};
  }
}

export function writeTextEdits(edits: TextEdits) {
  try {
    if (Object.keys(edits).length) window.localStorage.setItem(KEY, JSON.stringify(edits));
    else window.localStorage.removeItem(KEY);
  } catch {
    // Storage blocked: nothing is kept.
  }
  listeners.forEach(listener => listener());
}

/** An edit's values without its bookkeeping: whether it changes anything. */
export const editCount = (edits: TextEdits) => Object.values(edits).filter(edit => declarations(edit).length).length;

function declarations(edit: TextEdit): string[] {
  const size = typeof edit.size === "number" ? `${Number((edit.size / 16).toFixed(4))}rem` : edit.size ? `var(--text-${edit.size})` : "";
  const face = fontCss(edit.font);
  const colour = edit.colour?.startsWith("--") ? `var(${edit.colour})` : edit.colour;
  return [
    edit.role && `font: var(--${edit.role})`,
    edit.role && `letter-spacing: var(--${edit.role}-tracking)`,
    face && `font-family: ${face}`,
    size && `font-size: ${size}`,
    edit.weight !== undefined && `font-weight: ${edit.weight}`,
    edit.tracking !== undefined && `letter-spacing: ${edit.tracking}em`,
    edit.line !== undefined && `line-height: ${edit.line}`,
    colour && `color: ${colour}`,
  ].filter((line): line is string => Boolean(line));
}

/** Split a selector list on its top-level commas (not those inside :is() and the like). */
export function splitSelectors(selector: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < selector.length; index++) {
    const char = selector[index];
    if (char === "(" || char === "[") depth++;
    else if (char === ")" || char === "]") depth--;
    else if (char === "," && depth === 0) {
      parts.push(selector.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(selector.slice(start).trim());
  return parts.filter(Boolean);
}

/** The stylesheet the dev panel keeps on every page. */
export function textEditsCss(edits: TextEdits): string {
  return Object.entries(edits)
    .map(([selector, edit]) => {
      const lines = declarations(edit);
      if (!lines.length) return "";
      const scoped = splitSelectors(selector).map(part => `${part}:not([data-type-inspect-exempt] *)`).join(",\n");
      return `${scoped} {\n${lines.map(line => `  ${line} !important;`).join("\n")}\n}`;
    })
    .filter(Boolean)
    .join("\n");
}

/** What Copy CSS gives: each edit under a comment naming its source, without the dev-only parts. */
export function textEditsCopy(edits: TextEdits): string {
  return Object.values(edits)
    .map(edit => {
      const lines = declarations(edit);
      if (!lines.length) return "";
      if (edit.only !== undefined) {
        const within = edit.source ? ` (styled by ${edit.source})` : "";
        return `/* One text, "${edit.only}"${within}: give it its own class or role in its component */\n{\n${lines.map(line => `  ${line};`).join("\n")}\n}`;
      }
      const note = edit.component ? " (Silicon UI's own rule: move this into a token or ask Silicon UI; components aren't restyled here)" : "";
      const [file, ...rest] = edit.source.split(" ");
      return `/* ${file}${note} */\n${rest.join(" ") || edit.source} {\n${lines.map(line => `  ${line};`).join("\n")}\n}`;
    })
    .filter(Boolean)
    .join("\n\n");
}
