"use client";

/**
 * The type inspector and Edit text (Anil, 2026-11-06; editing 2026-11-09), development only: switched on in the dev
 * panel (⌥I). The cursor becomes a crosshair; hovering text outlines it and shows its role on the type scale
 * (styles/tokens.css), or "Off scale" with the nearest role and what differs, then its size, line-height, weight,
 * tracking and family. Clicking text opens an editor for the CSS rule that styles it (lib/text-edits.ts): role, face,
 * size, weight, tracking, line height and colour, applied to every text that rule styles, on every page, until reset;
 * or, with "Just this one", to the clicked text alone.
 * Other clicks in the page do nothing while it's on, so inspecting can't navigate or submit. Esc closes the editor, then
 * turns the inspector off.
 *
 * Roles are read from the real tokens through hidden probes, so there's no second copy of the scale to drift. Browsers
 * don't say which face rendered, so the family is the first name in the computed stack.
 */
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { ColorPicker } from "@teamofsilicons/silicon-ui/color-picker";
import { MorphSelect } from "@teamofsilicons/silicon-ui/morph-select";
import SegmentedControl from "@teamofsilicons/silicon-ui/segmented-control";
import { Select } from "@teamofsilicons/silicon-ui/select";
import { Slider } from "@teamofsilicons/silicon-ui/slider";
import { parseTextEdits, readTextEditsText, ROLES, subscribeTextEdits, TEXT_COLOURS, OPENAI_COLOURS, isListedColour, writeTextEdits, type TextEdit } from "@/lib/text-edits";
import { LAB_FONTS, STEPS } from "@/lib/type-lab";
import styles from "./type-inspector.module.css";

/** Anything marked with this (the dev tools) stays clickable and isn't inspected; open lists and pickers neither. */
const EXEMPT = "[data-type-inspect-exempt], [data-radix-popper-content-wrapper], [role='listbox']";
const GAP = 14;
const TYPE_PROPS = ["font", "font-size", "font-family", "font-weight", "letter-spacing", "line-height", "color"];

interface Measure {
  size: number;
  /** Line-height as a ratio of size; null for `normal`. */
  line: number | null;
  weight: number;
  /** Tracking in em. */
  tracking: number;
}

interface Reading extends Measure {
  role: string | null;
  near: string | null;
  lineLabel: string;
  family: string;
}

/** A CSS rule that styles the picked text: its selector, where it comes from, and whether it is Silicon UI's. */
interface Rule {
  selector: string;
  source: string;
  component: boolean;
}

interface Picked extends Measure {
  rules: Rule[];
  colour: string;
  /** The clicked text's own selector, for "Just this one". */
  path: string;
  /** The start of its words. */
  words: string;
}

/**
 * A selector for this one element: its tags and their places among siblings, from the nearest ancestor with an id, or
 * from <main>, or from <body>. Generated class names are left out, so the path reads plainly.
 */
function pathTo(element: Element): string {
  const steps: string[] = [];
  for (let node: Element | null = element; node && node !== document.documentElement; node = node.parentElement) {
    if (node.id && !/^[:\d]|radix/i.test(node.id)) {
      steps.unshift(`#${CSS.escape(node.id)}`);
      break;
    }
    const tag = node.tagName.toLowerCase();
    if (tag === "main" || tag === "body") {
      steps.unshift(tag);
      break;
    }
    const parent: Element | null = node.parentElement;
    const index = parent ? [...parent.children].filter(child => child.tagName === node!.tagName).indexOf(node) + 1 : 1;
    steps.unshift(`${tag}:nth-of-type(${index})`);
  }
  return steps.join(" > ");
}

function measure(style: CSSStyleDeclaration): Measure {
  const size = parseFloat(style.fontSize);
  const line = parseFloat(style.lineHeight);
  const tracking = parseFloat(style.letterSpacing);
  return { size, line: Number.isNaN(line) ? null : line / size, weight: Number(style.fontWeight), tracking: Number.isNaN(tracking) ? 0 : tracking / size };
}

function readRoles(): Record<string, Measure> {
  const out: Record<string, Measure> = {};
  for (const role of ROLES) {
    const probe = document.createElement("span");
    probe.hidden = true;
    probe.style.font = `var(--${role})`;
    probe.style.letterSpacing = `var(--${role}-tracking)`;
    document.body.append(probe);
    out[role] = measure(getComputedStyle(probe));
    probe.remove();
  }
  return out;
}

const round = (value: number, places = 2) => String(Number(value.toFixed(places)));

/** What differs from a role, in the words the readout uses. */
function differences(found: Measure, role: Measure): string[] {
  const out: string[] = [];
  if (Math.abs(found.size - role.size) > 0.1) out.push(`size ${round(found.size)}`);
  if (found.line === null || role.line === null ? found.line !== role.line : Math.abs(found.line - role.line) > 0.01) out.push(`line ${found.line === null ? "normal" : round(found.line)}, not ${role.line === null ? "normal" : round(role.line)}`);
  if (found.weight !== role.weight) out.push(`weight ${found.weight}, not ${role.weight}`);
  if (Math.abs(found.tracking - role.tracking) > 0.002) out.push(`tracking ${round(found.tracking, 3)}em, not ${round(role.tracking, 3)}em`);
  return out;
}

function read(element: Element, roles: Record<string, Measure>): Reading {
  const style = getComputedStyle(element);
  const found = measure(style);
  const family = style.fontFamily.split(",")[0]?.trim().replace(/^["']|["']$/g, "") ?? "";
  const lineLabel = found.line === null ? "normal" : `${round(found.line * found.size)} px (${round(found.line)})`;
  const reading: Reading = { ...found, role: null, near: null, lineLabel, family };

  let fewest = Infinity;
  for (const [name, role] of Object.entries(roles)) {
    const diff = differences(found, role);
    if (!diff.length) return { ...reading, role: name, near: null };
    if (Math.abs(found.size - role.size) <= 0.1 && diff.length < fewest) {
      fewest = diff.length;
      reading.near = `Near ${name}: ${diff.join(", ")}`;
    }
  }
  return reading;
}

/** The text under a point: the element holding it and the box to outline, or null over padding, icons and the like. */
function textAt(x: number, y: number): { element: Element; box: DOMRect } | null {
  const hit = document.elementFromPoint(x, y);
  if (!hit || hit.closest(EXEMPT)) return null;
  if (hit instanceof HTMLInputElement || hit instanceof HTMLTextAreaElement) {
    return hit.value || hit.placeholder ? { element: hit, box: hit.getBoundingClientRect() } : null;
  }
  const range = document.createRange();
  for (let element: Element | null = hit; element && element !== document.body; element = element.parentElement) {
    for (const node of element.childNodes) {
      if (node.nodeType !== Node.TEXT_NODE || !node.textContent?.trim()) continue;
      range.selectNodeContents(node);
      const over = Array.from(range.getClientRects()).some(rect => x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom);
      if (over) return { element, box: range.getBoundingClientRect() };
    }
  }
  return null;
}

/** Every style rule on the page, through @media, @layer and @supports, skipping sheets the browser won't let us read. */
function* styleRules(rules: CSSRuleList): Generator<CSSStyleRule> {
  for (const rule of rules) {
    if (rule instanceof CSSStyleRule) yield rule;
    else if (rule instanceof CSSMediaRule && !window.matchMedia(rule.conditionText).matches) continue;
    else if ("cssRules" in rule) yield* styleRules((rule as CSSGroupingRule).cssRules);
  }
}

/**
 * Where each CSS module's classes come from: Next's development stylesheets mark every source file with a comment
 * (`/* [project]/components/account/silicons/silicons.module.css [app-client] (css) *\/`), and its classes follow,
 * named `silicons-module__mgezRW__sectionTitle`. Read once per inspector, from the same-origin sheets.
 */
let sources: Map<string, string> | null = null;
async function loadSources() {
  if (sources) return;
  const found = new Map<string, string>();
  for (const sheet of document.styleSheets) {
    if (!sheet.href || new URL(sheet.href).origin !== location.origin) continue;
    try {
      const text = await (await fetch(sheet.href)).text();
      const parts = text.split(/\/\* \[project\]\/(\S+) [^*]*\*\//);
      for (let index = 1; index < parts.length; index += 2) {
        for (const match of parts[index + 1]!.matchAll(/\.([\w-]+-module__[\w-]+?)__[\w-]+/g)) found.set(match[1]!, parts[index]!);
      }
    } catch {
      // Not readable: classes keep their generated names.
    }
  }
  sources = found;
}

/** A rule's selector in the words of its source: `components/…/silicons.module.css .sectionTitle`, and whether it is Silicon UI's. */
function describe(selector: string): Rule {
  let file: string | undefined;
  const clean = selector.replace(/\.([\w-]+-module__[\w-]+?)__([\w-]+)/g, (whole, prefix: string, name: string) => {
    const path = sources?.get(prefix);
    if (!path) return whole;
    file ??= path;
    return `.${name}`;
  });
  const component = file?.includes("@teamofsilicons") ?? false;
  const short = file && component ? `silicon-ui/${file.split("@teamofsilicons/silicon-ui/")[1] ?? file}` : file;
  return { selector, source: short ? `${short} ${clean}` : clean, component };
}

/**
 * The rules that set type or colour on the text, the element's own first and the nearest ancestor's when it has none.
 * Only rules with a class: tag and :root rules style everything. The last one found wins in the cascade, so it is first.
 */
function rulesFor(element: Element): Rule[] {
  for (let node: Element | null = element; node && node !== document.body; node = node.parentElement) {
    const found: Rule[] = [];
    for (const sheet of document.styleSheets) {
      let rules: CSSRuleList;
      try {
        rules = sheet.cssRules;
      } catch {
        continue;
      }
      for (const rule of styleRules(rules)) {
        if (!rule.selectorText.includes(".") || !TYPE_PROPS.some(prop => rule.style.getPropertyValue(prop))) continue;
        let matches = false;
        try {
          matches = node.matches(rule.selectorText);
        } catch {
          continue;
        }
        if (!matches) continue;
        found.push(describe(rule.selectorText));
      }
    }
    if (found.length) return found.reverse().filter((rule, index, all) => all.findIndex(other => other.selector === rule.selector) === index);
  }
  return [];
}

export function TypeInspector({ onExit }: { onExit: () => void }) {
  const [reading, setReading] = useState<Reading | null>(null);
  const [picked, setPicked] = useState<Picked | null>(null);
  const outline = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const point = useRef({ x: 0, y: 0 });
  const pickedRef = useRef<Picked | null>(null);
  useEffect(() => {
    pickedRef.current = picked;
  }, [picked]);

  const place = () => {
    const el = card.current;
    if (!el) return;
    const { x, y } = point.current;
    const left = x + GAP + el.offsetWidth > window.innerWidth ? x - GAP - el.offsetWidth : x + GAP;
    const top = y + GAP + el.offsetHeight > window.innerHeight ? y - GAP - el.offsetHeight : y + GAP;
    el.style.transform = `translate(${Math.max(GAP, left)}px, ${Math.max(GAP, top)}px)`;
  };

  useLayoutEffect(place, [reading]);

  useEffect(() => {
    const roles = readRoles();
    void loadSources();
    const root = document.documentElement;
    root.setAttribute("data-type-inspect", "");
    let frame = 0;

    const update = () => {
      frame = 0;
      const found = textAt(point.current.x, point.current.y);
      if (outline.current) {
        outline.current.hidden = !found;
        if (found) {
          outline.current.style.transform = `translate(${found.box.left}px, ${found.box.top}px)`;
          outline.current.style.width = `${found.box.width}px`;
          outline.current.style.height = `${found.box.height}px`;
        }
      }
      setReading(found ? read(found.element, roles) : null);
      place();
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    const move = (event: PointerEvent) => {
      point.current = { x: event.clientX, y: event.clientY };
      schedule();
    };
    const swallow = (event: Event) => {
      if (event.target instanceof Element && event.target.closest(EXEMPT)) return;
      event.preventDefault();
      event.stopPropagation();
    };
    const pick = (event: MouseEvent) => {
      if (event.target instanceof Element && event.target.closest(EXEMPT)) return;
      swallow(event);
      const found = textAt(event.clientX, event.clientY);
      if (!found) return;
      const style = getComputedStyle(found.element);
      const words = (found.element.textContent ?? "").trim().replace(/\s+/g, " ");
      setPicked({ ...measure(style), rules: rulesFor(found.element), colour: style.color, path: pathTo(found.element), words: words.length > 32 ? `${words.slice(0, 32)}…` : words });
    };
    const key = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (event.target instanceof Element && event.target.closest("[role='listbox'], [data-radix-popper-content-wrapper]")) return;
      event.stopPropagation();
      if (pickedRef.current) setPicked(null);
      else onExit();
    };

    // On window, so they run before anything on document: Radix closes a dialog on Esc in document's capture phase.
    window.addEventListener("pointermove", move, true);
    window.addEventListener("scroll", schedule, true);
    window.addEventListener("pointerdown", swallow, true);
    window.addEventListener("click", pick, true);
    window.addEventListener("keydown", key, true);
    return () => {
      cancelAnimationFrame(frame);
      root.removeAttribute("data-type-inspect");
      window.removeEventListener("pointermove", move, true);
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("pointerdown", swallow, true);
      window.removeEventListener("click", pick, true);
      window.removeEventListener("keydown", key, true);
    };
  }, [onExit]);

  return createPortal(
    <>
      <div ref={outline} className={styles.outline} hidden />
      <div ref={card} className={styles.card} hidden={!reading} data-off={reading && !reading.role ? "" : undefined} data-type-inspect-exempt="">
        {reading ? (
          <>
            <p className={styles.role}>{reading.role ?? "Off scale"}</p>
            {reading.near ? <p className={styles.near}>{reading.near}</p> : null}
            <dl className={styles.facts}>
              <dt>Size</dt><dd>{round(reading.size)} px</dd>
              <dt>Line</dt><dd>{reading.lineLabel}</dd>
              <dt>Weight</dt><dd>{reading.weight}</dd>
              <dt>Tracking</dt><dd>{round(reading.tracking, 3)}em</dd>
              <dt>Family</dt><dd>{reading.family}</dd>
            </dl>
            <p className={styles.near}>Click to edit</p>
          </>
        ) : null}
      </div>
      {picked ? <TextEditor key={picked.path} picked={picked} onClose={() => setPicked(null)} /> : null}
    </>,
    document.body,
  );
}

const ROLE_OPTIONS = [{ value: "none", label: "Its own" }, ...ROLES.map(role => ({ value: role, label: role }))];
const FONT_OPTIONS = LAB_FONTS.map(({ value, label }) => ({ value, label }));
const SIZE_OPTIONS = [{ value: "own", label: "Its own" }, ...STEPS.map(step => ({ value: step, label: `--text-${step}` })), { value: "custom", label: "Custom" }];
const swatch = (colour: string) => <span className={styles.swatch} style={{ background: colour }} aria-hidden="true" />;
/** Each colour by its plain name, with a dot of it; "Its own" and Custom show the text's current colour. */
const colourOptions = (own: string, custom: string) => [
  { value: "own", label: "Its own", icon: swatch(own) },
  { label: "Ours", options: TEXT_COLOURS.map(({ token, name }) => ({ value: token, label: name, icon: swatch(`var(${token})`) })) },
  { label: "OpenAI platform", options: OPENAI_COLOURS.map(({ value, name }) => ({ value, label: name, meta: value, icon: swatch(value) })) },
  { value: "custom", label: "Custom", icon: swatch(custom) },
];

/** The editor for one rule: everything it sets on its text, kept in lib/text-edits.ts. */
const SCOPE_OPTIONS = [{ value: "all", label: "Every one like it" }, { value: "one", label: "Just this one" }];

function TextEditor({ picked, onClose }: { picked: Picked; onClose: () => void }) {
  const edits = parseTextEdits(useSyncExternalStore(subscribeTextEdits, readTextEditsText, () => "{}"));
  const [ruleSelector, setRuleSelector] = useState(picked.rules[0]?.selector ?? "");
  const rule = picked.rules.find(item => item.selector === ruleSelector);
  const [scope, setScope] = useState<"all" | "one">(!rule || edits[picked.path] ? "one" : "all");
  const selector = scope === "one" ? picked.path : ruleSelector;
  const fresh: TextEdit = scope === "one" ? { source: rule?.source ?? "", only: picked.words } : { source: rule?.source ?? ruleSelector, component: rule?.component };
  const edit: TextEdit = edits[selector] ?? fresh;
  const [customSize, setCustomSize] = useState(typeof edit.size === "number");
  const [customColour, setCustomColour] = useState(Boolean(edit.colour && !isListedColour(edit.colour)));
  const count = (() => {
    try {
      return document.querySelectorAll(selector).length;
    } catch {
      return 0;
    }
  })();

  const change = (next: Partial<TextEdit>) => {
    const merged: TextEdit = { ...edit, ...fresh, ...next };
    for (const name of Object.keys(merged) as (keyof TextEdit)[]) if (merged[name] === undefined) delete merged[name];
    writeTextEdits({ ...edits, [selector]: merged });
  };
  const reset = () => {
    const rest = { ...edits };
    delete rest[selector];
    writeTextEdits(rest);
    setCustomSize(false);
    setCustomColour(false);
  };

  const sizeValue = customSize ? "custom" : typeof edit.size === "string" ? edit.size : "own";
  const colourValue = customColour ? "custom" : edit.colour ?? "own";
  return (
    <aside className={styles.editor} aria-label="Edit text" data-type-inspect-exempt="">
      <div className={styles.head}>
        <p className={styles.title}>Edit text</p>
        <div className={styles.actions}>
          <Button variant="ghost" size="xs" onClick={reset} disabled={!edits[selector]}>Reset This Text</Button>
          <Button variant="ghost" size="xs" aria-label="Done" onClick={onClose}><X size={14} strokeWidth={2} aria-hidden="true" /></Button>
        </div>
      </div>
      <div className={styles.body}>
        <div className={styles.source}>
          {rule ? <SegmentedControl size="xs" label="Change" options={SCOPE_OPTIONS} value={scope} onValueChange={value => setScope(value as "all" | "one")} /> : null}
          {scope === "all" && picked.rules.length > 1
            ? <Select label="Rule" options={picked.rules.map(item => ({ value: item.selector, label: item.source }))} value={ruleSelector} onValueChange={setRuleSelector} />
            : <code className={styles.code}>{scope === "one" ? `“${picked.words}”${rule ? ` in ${rule.source}` : ""}` : rule?.source}</code>}
          <div className={styles.badges}>
            <span className={styles.hint}>{scope === "one" ? (rule ? "This text only" : "No rule styles this text, so only it changes") : count === 1 ? "1 text on this page" : `${count} texts on this page`}</span>
            {scope === "all" && rule?.component ? <Badge size="sm" tone="warning">Silicon UI component</Badge> : null}
          </div>
        </div>
        <Select label="Role" options={ROLE_OPTIONS} value={edit.role ?? "none"} onValueChange={value => change({ role: value === "none" ? undefined : (value as TextEdit["role"]) })} />
        <Select label="Font" options={FONT_OPTIONS} value={edit.font ?? "page"} onValueChange={value => change({ font: value === "page" ? undefined : value })} />
        <Select label="Size" options={SIZE_OPTIONS} value={sizeValue} onValueChange={value => {
          setCustomSize(value === "custom");
          change({ size: value === "own" ? undefined : value === "custom" ? Math.round(picked.size) : value });
        }} />
        {customSize ? <Slider label="Custom size" min={8} max={96} step={1} showValue format={value => `${value} px`} value={typeof edit.size === "number" ? edit.size : Math.round(picked.size)} onValueChange={size => change({ size })} /> : null}
        <Slider label="Weight" min={200} max={900} step={10} showValue value={edit.weight ?? picked.weight} onValueChange={weight => change({ weight })} />
        <Slider label="Letter spacing" min={-0.08} max={0.08} step={0.0025} showValue format={value => `${value.toFixed(3)} em`} value={edit.tracking ?? Number(picked.tracking.toFixed(3))} onValueChange={tracking => change({ tracking })} />
        <Slider label="Line height" min={0.8} max={2.2} step={0.05} showValue format={value => value.toFixed(2)} value={edit.line ?? Number((picked.line ?? 1.4).toFixed(2))} onValueChange={line => change({ line })} />
        <MorphSelect label="Colour" searchable={false} items={colourOptions(picked.colour, customColour && edit.colour ? edit.colour : picked.colour)} value={colourValue} onValueChange={value => {
          setCustomColour(value === "custom");
          change({ colour: value === "own" ? undefined : value === "custom" ? picked.colour : value });
        }} />
        {customColour ? <ColorPicker label="Custom colour" value={edit.colour ?? picked.colour} onValueChange={colour => change({ colour })} /> : null}
        {scope === "all" && rule?.component ? <p className={styles.hint}>Silicon UI&rsquo;s own rule: try it here, then move it into a token or ask Silicon UI. Components aren&rsquo;t restyled in code.</p> : null}
      </div>
    </aside>
  );
}
