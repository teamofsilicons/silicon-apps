"use client";

/**
 * The type lab (Anil, 2026-11-09; the font lab of 2026-11-07, grown), development only: a small panel that sets Silicon
 * UI's type tokens on the real page while it stays in view, so every component and every page retypes at once. A preset
 * to start from, then the text and title faces (with Inter's and Hanken's cv01 and ss03 alternates), the nine size steps,
 * tracking and line height for display and body, and the three weights. No colours. Sliders start at the page's own
 * tokens. The values live in lib/type-lab.ts and the dev panel keeps their stylesheet on every page until Reset, so closing
 * the lab (its close button or Esc) leaves the change on. Copy CSS gives a :root block for styles/tokens.css.
 */
import { useEffect, useState, useSyncExternalStore } from "react";
import { X } from "lucide-react";
import { createPortal } from "react-dom";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { Checkbox } from "@teamofsilicons/silicon-ui/checkbox";
import { CopyButton } from "@teamofsilicons/silicon-ui/copy-button";
import { Select } from "@teamofsilicons/silicon-ui/select";
import { Slider } from "@teamofsilicons/silicon-ui/slider";
import { LAB_FONTS, parseTypeLab, PRESETS, readTokenDefaults, readTypeLabText, STEPS, subscribeTypeLab, typeLabCss, writeTypeLab, type TypeLabValues } from "@/lib/type-lab";
import styles from "./type-lab.module.css";

const FONT_OPTIONS = LAB_FONTS.map(({ value, label }) => ({ value, label }));
const PRESET_OPTIONS = [{ value: "custom", label: "Custom" }, ...PRESETS.map(({ value, label }) => ({ value, label }))];
/** The dev panel's name for the lab's stylesheet (dev-states.tsx). */
export const TYPE_LAB_SHEET = "1-type-lab";

const em = (value: number) => `${value.toFixed(3)} em`;

export function TypeLab({ onClose }: { onClose: () => void }) {
  const values = parseTypeLab(useSyncExternalStore(subscribeTypeLab, readTypeLabText, () => "{}"));
  const [page, setPage] = useState(() => readTokenDefaults(TYPE_LAB_SHEET));
  const css = typeLabCss(values);
  const change = (next: TypeLabValues) => writeTypeLab({ ...values, ...next });
  const preset = PRESETS.find(choice => JSON.stringify(choice.values) === JSON.stringify(values))?.value ?? "custom";
  const reset = () => {
    setPage(readTokenDefaults(TYPE_LAB_SHEET));
    writeTypeLab({});
  };

  // Esc closes the lab (its changes stay on), except when it closes an open list or another layer first.
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (event.target instanceof Element && event.target.closest('[role="listbox"], [role="menu"], [role="dialog"]')) return;
      onClose();
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [onClose]);

  return createPortal(
    <aside className={styles.panel} aria-label="Type lab" data-type-inspect-exempt="">
      <div className={styles.head}>
        <p className={styles.title}>Type lab</p>
        <div className={styles.actions}>
          <Button variant="ghost" size="xs" disabled={!css} onClick={reset}>Reset</Button>
          <Button variant="ghost" size="xs" aria-label="Close type lab" onClick={onClose}>
            <X size={14} strokeWidth={2} aria-hidden="true" />
          </Button>
        </div>
      </div>
      <div className={styles.body}>
        <section className={styles.group} aria-label="Start from">
          <Select label="Start from" options={PRESET_OPTIONS} value={preset} onValueChange={value => {
            const choice = PRESETS.find(item => item.value === value);
            if (choice) writeTypeLab(choice.values);
          }} />
        </section>
        <section className={styles.group} aria-label="Faces">
          <p className={styles.groupTitle}>Faces</p>
          <Select label="Text" options={FONT_OPTIONS} value={values.textFont ?? "page"} onValueChange={textFont => change({ textFont })} />
          <Select label="Titles" options={FONT_OPTIONS} disabled={values.sameFont === true} value={values.sameFont ? values.textFont ?? "page" : values.titleFont ?? "page"} onValueChange={titleFont => change({ titleFont })} />
          <Checkbox label="Titles use the text face" checked={values.sameFont === true} onCheckedChange={on => change({ sameFont: on === true || undefined })} />
          <Checkbox label="Alternates (cv01, ss03)" checked={values.features === true} onCheckedChange={on => change({ features: on === true || undefined })} />
        </section>
        <section className={styles.group} aria-label="Sizes">
          <p className={styles.groupTitle}>Sizes</p>
          {STEPS.map(step => (
            <Slider key={step} label={`--text-${step}`} min={8} max={96} step={1} showValue format={value => `${value} px`}
              value={values.sizes?.[step] ?? page.sizes[step]}
              onValueChange={size => change({ sizes: { ...values.sizes, [step]: size } })} />
          ))}
        </section>
        <section className={styles.group} aria-label="Spacing">
          <p className={styles.groupTitle}>Tracking and line height</p>
          <Slider label="Display tracking" min={-0.08} max={0.04} step={0.0025} showValue format={em} value={values.trackingDisplay ?? page.trackingDisplay} onValueChange={trackingDisplay => change({ trackingDisplay })} />
          <Slider label="Body tracking" min={-0.05} max={0.05} step={0.0025} showValue format={em} value={values.trackingBody ?? page.trackingBody} onValueChange={trackingBody => change({ trackingBody })} />
          <Slider label="Display line height" min={0.9} max={1.6} step={0.05} showValue format={value => value.toFixed(2)} value={values.leadingDisplay ?? page.leadingDisplay} onValueChange={leadingDisplay => change({ leadingDisplay })} />
          <Slider label="Body line height" min={1} max={2} step={0.05} showValue format={value => value.toFixed(2)} value={values.leadingBody ?? page.leadingBody} onValueChange={leadingBody => change({ leadingBody })} />
        </section>
        <section className={styles.group} aria-label="Weights">
          <p className={styles.groupTitle}>Weights</p>
          <Slider label="Titles" min={300} max={900} step={10} showValue value={values.weightTitle ?? page.weightTitle} onValueChange={weightTitle => change({ weightTitle })} />
          <Slider label="Medium" min={300} max={900} step={10} showValue value={values.weightMedium ?? page.weightMedium} onValueChange={weightMedium => change({ weightMedium })} />
          <Slider label="Regular" min={200} max={700} step={10} showValue value={values.weightRegular ?? page.weightRegular} onValueChange={weightRegular => change({ weightRegular })} />
          <span className={styles.hint}>Our text only: Silicon UI components keep their own 500 and 400. Edit text can change one kind of component text.</span>
        </section>
      </div>
      <div className={styles.foot}>
        <span className={styles.hint}>{css ? "On every page until Reset" : "Nothing changed"}</span>
        <CopyButton value={css} label="Copy CSS" disabled={!css} />
      </div>
    </aside>,
    document.body,
  );
}
