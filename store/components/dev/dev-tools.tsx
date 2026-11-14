"use client";

/**
 * The dev tools panel, development only (`next dev`): the account site's (silicon-accounts web,
 * components/foundation/dev/dev-states.tsx, 2026-11-13), copied 2026-11-14 without its Data group, whose states are the
 * account site's own. A small round button at the lower right of every page opens a compact panel in two groups:
 *
 * - Look: Density (the store is compact like the account site; Comfortable puts the account site's earlier, roomier
 *   scale back, to compare), the type lab (./type-lab.tsx, ⌥F), which sets Silicon UI's type tokens for the whole
 *   interface, and the text edits made by clicking text with Edit text on (lib/text-edits.ts), with their count, Copy
 *   CSS and Clear. All stay on across pages and reloads until reset; their stylesheets are kept in <head> here, the
 *   type lab first so a text's own edit wins.
 * - Inspect: Edit text, the type inspector that edits what it's clicked on (./type-inspector.tsx, ⌥I), and the marquee
 *   spacing inspector (./spacing-inspector.tsx, ⌥S); one at a time, and turning one on closes the panel so the page is
 *   free to use.
 *
 * The button's dot counts what is changed, or shows a crosshair while an inspector is on. Reset All puts everything
 * back.
 */
import { useEffect, useState, useSyncExternalStore } from "react";
import { Crosshair, FlaskConical } from "lucide-react";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { CopyButton } from "@teamofsilicons/silicon-ui/copy-button";
import { Popover, PopoverContent, PopoverTrigger } from "@teamofsilicons/silicon-ui/popover";
import SegmentedControl from "@teamofsilicons/silicon-ui/segmented-control";
import { Switch } from "@teamofsilicons/silicon-ui/switch";
import { editCount, parseTextEdits, readTextEditsText, subscribeTextEdits, textEditsCopy, textEditsCss, writeTextEdits } from "@/lib/text-edits";
import { parseTypeLab, readTypeLabText, subscribeTypeLab, typeLabCss, writeTypeLab } from "@/lib/type-lab";
import { useDialogPassThrough } from "./dialog-pass-through";
import { SpacingInspector } from "./spacing-inspector";
import { TypeInspector } from "./type-inspector";
import { TYPE_LAB_SHEET, TypeLab } from "./type-lab";
import styles from "./dev-tools.module.css";

type Inspector = "type" | "spacing" | null;


/**
 * A stylesheet kept in <head> while `css` has anything in it. Each call owns one element, made on first use, so the
 * order of the calls is the order of the sheets.
 */
function useHeadStyle(name: string, css: string) {
  useEffect(() => {
    if (!css) return;
    const sheet = document.createElement("style");
    sheet.dataset.devStyle = name;
    sheet.textContent = css;
    const later = [...document.head.querySelectorAll<HTMLStyleElement>("style[data-dev-style]")].find(other => other.dataset.devStyle! > name);
    document.head.insertBefore(sheet, later ?? null);
    return () => sheet.remove();
  }, [name, css]);
}

/**
 * Density: the store is compact like the account site (styles/tokens.css); this puts the account site's comfortable
 * scale back on <html data-density="comfortable">, kept in this browser, to compare.
 */
const DENSITY_KEY = "store.dev.density";
const densityListeners = new Set<() => void>();
const subscribeDensity = (listener: () => void) => {
  densityListeners.add(listener);
  return () => densityListeners.delete(listener);
};
const readDensity = () => {
  try {
    return window.localStorage.getItem(DENSITY_KEY) === "comfortable" ? "comfortable" : "compact";
  } catch {
    return "compact";
  }
};
const writeDensity = (density: string) => {
  try {
    if (density === "comfortable") window.localStorage.setItem(DENSITY_KEY, density);
    else window.localStorage.removeItem(DENSITY_KEY);
  } catch {
    // Storage blocked: the choice lasts until the page reloads.
  }
  densityListeners.forEach(listener => listener());
};
const DENSITY_OPTIONS = [{ value: "compact", label: "Compact" }, { value: "comfortable", label: "Comfortable" }];

export function DevTools() {
  useDialogPassThrough();
  const typeLab = parseTypeLab(useSyncExternalStore(subscribeTypeLab, readTypeLabText, () => "{}"));
  const textEdits = parseTextEdits(useSyncExternalStore(subscribeTextEdits, readTextEditsText, () => "{}"));
  const density = useSyncExternalStore(subscribeDensity, readDensity, () => "compact");
  const [open, setOpen] = useState(false);
  const [inspector, setInspector] = useState<Inspector>(null);
  const [labOpen, setLabOpen] = useState(false);
  // The React Compiler keeps these two stable, so the inspectors' and the lab's listeners aren't re-added each render.
  const stopInspecting = () => setInspector(null);
  const closeLab = () => setLabOpen(false);

  // Sorted by name in <head>: "1-type-lab" before "2-text-edits", so a text's own edit wins over the tokens.
  const labCss = typeLabCss(typeLab);
  const editsCss = textEditsCss(textEdits);
  const edits = editCount(textEdits);
  useHeadStyle(TYPE_LAB_SHEET, labCss);
  useHeadStyle("2-text-edits", editsCss);

  useEffect(() => {
    if (density === "comfortable") document.documentElement.dataset.density = density;
    else delete document.documentElement.dataset.density;
  }, [density]);

  // ⌥I, ⌥S and ⌥F from anywhere: an open dialog blocks the flask button, and dialogs have type to inspect too.
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (!event.altKey || !["KeyI", "KeyS", "KeyF"].includes(event.code)) return;
      event.preventDefault();
      setOpen(false);
      if (event.code === "KeyF") setLabOpen(on => !on);
      else {
        const next = event.code === "KeyI" ? "type" : "spacing";
        setInspector(current => (current === next ? null : next));
      }
    };
    document.addEventListener("keydown", key, true);
    return () => document.removeEventListener("keydown", key, true);
  }, []);

  const changed = (labCss ? 1 : 0) + edits + (density === "comfortable" ? 1 : 0);

  const resetAll = () => {
    writeTypeLab({});
    writeTextEdits({});
    writeDensity("compact");
  };
  const inspect = (which: Exclude<Inspector, null>, on: boolean) => {
    setInspector(on ? which : null);
    if (on) setOpen(false);
  };

  return (
    <div className={styles.dock} data-type-inspect-exempt="">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button variant="soft" size="sm" className={styles.trigger} aria-label={changed ? `Dev tools, ${changed} changed` : "Dev tools"}>
            <FlaskConical size={16} strokeWidth={2} aria-hidden="true" />
          </Button>
        </PopoverTrigger>
        {inspector ? <span className={styles.dot} aria-hidden="true"><Crosshair size={12} strokeWidth={2} /></span> : changed ? <span className={styles.dot} aria-hidden="true">{changed}</span> : null}
        <PopoverContent side="top" align="end" sideOffset={8} className={styles.panel} data-type-inspect-exempt="">
          <div className={styles.head}>
            <p className={styles.title}>Dev tools</p>
            {changed ? <Button variant="ghost" size="xs" onClick={resetAll}>Reset All</Button> : <span className={styles.hint}>Nothing changed</span>}
          </div>

          <div className={styles.body}>
            <section className={styles.group} aria-labelledby="dev-look">
              <div className={styles.groupHead}>
                <p id="dev-look" className={styles.groupTitle}>Look</p>
                <span className={styles.hint}>Kept until reset</span>
              </div>
              <div className={styles.row}>
                <span className={styles.label}>Density</span>
                <SegmentedControl size="xs" label="Density" options={DENSITY_OPTIONS} value={density} onValueChange={writeDensity} />
              </div>
              <div className={styles.row}>
                <span className={styles.label}>Typography</span>
                <div className={styles.buttons}>
                  <Button variant="soft" size="xs" onClick={() => {
                    setLabOpen(on => !on);
                    setOpen(false);
                  }}>{labOpen ? "Close" : "Open"}</Button>
                  {labCss ? <Badge size="sm" tone="info">On</Badge> : null}
                  <kbd className={styles.key}>⌥F</kbd>
                </div>
              </div>
              <div className={styles.row}>
                <span className={styles.label}>Text edits</span>
                <div className={styles.buttons}>
                  {edits ? (
                    <>
                      <Badge size="sm" tone="info">{edits}</Badge>
                      <CopyButton value={textEditsCopy(textEdits)} label="Copy CSS" />
                      <Button variant="ghost" size="xs" onClick={() => writeTextEdits({})}>Clear</Button>
                    </>
                  ) : <span className={styles.hint}>Click text with Edit text on</span>}
                </div>
              </div>
            </section>

            <section className={styles.group} aria-labelledby="dev-inspect">
              <div className={styles.groupHead}>
                <p id="dev-inspect" className={styles.groupTitle}>Inspect</p>
                <span className={styles.hint}>Esc to stop</span>
              </div>
              <div className={styles.switchRow}>
                <Switch label="Edit text" checked={inspector === "type"} onCheckedChange={on => inspect("type", on)} />
                <kbd className={styles.key}>⌥I</kbd>
              </div>
              <div className={styles.switchRow}>
                <Switch label="Spacing" checked={inspector === "spacing"} onCheckedChange={on => inspect("spacing", on)} />
                <kbd className={styles.key}>⌥S</kbd>
              </div>
            </section>
          </div>
        </PopoverContent>
      </Popover>
      {inspector === "type" ? <TypeInspector onExit={stopInspecting} /> : null}
      {inspector === "spacing" ? <SpacingInspector onExit={stopInspecting} /> : null}
      {labOpen ? <TypeLab onClose={closeLab} /> : null}
    </div>
  );
}
