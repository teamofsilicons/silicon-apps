"use client";

/**
 * The spacing inspector (Anil, 2026-11-08), development only: switched on in the dev states panel or with ⌥S. The cursor
 * becomes a crosshair; drag a marquee over part of the page and everything it touches is measured inside it:
 *
 * - each box's outline, and its size (w × h) when it holds no text and no other boxes (an avatar, an icon);
 * - its corner radius ("r 20") when it has one;
 * - its padding, shaded with the content box dashed, and the value ("16", or one per side when they differ);
 * - the space between neighbouring boxes in a parent (a flex or grid gap, or margins), shaded and labelled;
 * - text, outlined to the words themselves.
 *
 * The marquee shows its own size at its lower right. Elements that hold the whole marquee (the page around it) aren't
 * measured, so a selection reads like a redline of what's in it. The drawing is clipped to the marquee and redrawn on
 * scroll and resize. Clicks in the page do nothing while it's on; a click without a drag clears the selection; Esc
 * turns it off.
 */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import styles from "./spacing-inspector.module.css";

/** Anything marked with this (the dev panel) stays clickable and isn't measured. */
const EXEMPT = "[data-type-inspect-exempt]";
/** A drag shorter than this is a click. */
const MIN_DRAG = 4;
/** Enough for a dense card without drowning the page in labels. */
const MAX_BOXES = 160;

interface Rect { x: number; y: number; w: number; h: number }
interface Shape { kind: "box" | "content" | "pad" | "gap" | "text"; rect: Rect }
interface Label { text: string; x: number; y: number; kind: "radius" | "pad" | "gap" | "hgap" | "size" }
interface Drawing { shapes: Shape[]; labels: Label[] }

const px = (value: string) => parseFloat(value) || 0;
const fmt = (value: number) => String(Math.round(value * 10) / 10);
const toRect = (box: DOMRect): Rect => ({ x: box.left, y: box.top, w: box.width, h: box.height });
const touches = (a: Rect, b: Rect) => a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
const holds = (outer: Rect, inner: Rect) => outer.x <= inner.x && outer.y <= inner.y && outer.x + outer.w >= inner.x + inner.w && outer.y + outer.h >= inner.y + inner.h;

function visible(element: Element, style: CSSStyleDeclaration, box: DOMRect) {
  if (box.width < 2 || box.height < 2) return false;
  if (style.visibility === "hidden" || style.display === "contents" || px(style.opacity) === 0 && style.opacity !== "") return false;
  return !element.closest(EXEMPT) && !element.closest("[data-spacing-inspector]");
}

/** Direct text in an element, as the boxes of its words. */
function textBoxes(element: Element): Rect[] {
  const out: Rect[] = [];
  const range = document.createRange();
  for (const node of element.childNodes) {
    if (node.nodeType !== Node.TEXT_NODE || !node.textContent?.trim()) continue;
    range.selectNodeContents(node);
    const box = range.getBoundingClientRect();
    if (box.width >= 1 && box.height >= 1) out.push(toRect(box));
  }
  return out;
}

function measure(area: Rect): Drawing {
  const shapes: Shape[] = [];
  const labels: Label[] = [];
  // A label whose spot is outside the marquee is pulled to its nearest edge, so a box cut by the marquee still says its values.
  const label = (text: string, x: number, y: number, kind: Label["kind"]) => {
    labels.push({ text, x: Math.min(Math.max(x, area.x), area.x + area.w), y: Math.min(Math.max(y, area.y), area.y + area.h), kind });
  };
  const picked = new Set<Element>();

  for (const element of document.body.querySelectorAll("*")) {
    if (picked.size >= MAX_BOXES) break;
    if (element instanceof SVGElement && !(element instanceof SVGSVGElement)) continue;
    const box = element.getBoundingClientRect();
    const rect = toRect(box);
    if (!touches(rect, area) || holds(rect, area)) continue;
    const style = getComputedStyle(element);
    if (!visible(element, style, box)) continue;
    picked.add(element);

    shapes.push({ kind: "box", rect });

    // Radius: the top-left corner's, which is the one labelled.
    const radius = px(style.borderTopLeftRadius);
    if (radius > 0 && radius < Math.min(rect.w, rect.h) / 2) label(`r ${fmt(radius)}`, rect.x + 4, rect.y + 4, "radius");

    // Padding: shaded, with the content box dashed.
    const pad = { top: px(style.paddingTop), right: px(style.paddingRight), bottom: px(style.paddingBottom), left: px(style.paddingLeft) };
    const border = { top: px(style.borderTopWidth), right: px(style.borderRightWidth), bottom: px(style.borderBottomWidth), left: px(style.borderLeftWidth) };
    if (pad.top || pad.right || pad.bottom || pad.left) {
      const outer = { x: rect.x + border.left, y: rect.y + border.top, w: rect.w - border.left - border.right, h: rect.h - border.top - border.bottom };
      const content = { x: outer.x + pad.left, y: outer.y + pad.top, w: outer.w - pad.left - pad.right, h: outer.h - pad.top - pad.bottom };
      shapes.push({ kind: "pad", rect: outer }, { kind: "content", rect: content });
      const same = pad.top === pad.right && pad.right === pad.bottom && pad.bottom === pad.left;
      if (same) label(fmt(pad.top), content.x + content.w, outer.y + pad.top / 2, "pad");
      else {
        if (pad.top) label(fmt(pad.top), content.x + content.w / 2, outer.y + pad.top / 2, "pad");
        if (pad.bottom) label(fmt(pad.bottom), content.x + content.w / 2, content.y + content.h + pad.bottom / 2, "pad");
        if (pad.left) label(fmt(pad.left), outer.x + pad.left / 2, content.y + content.h / 2, "pad");
        if (pad.right) label(fmt(pad.right), content.x + content.w + pad.right / 2, content.y + content.h / 2, "pad");
      }
    }

    // Text, to the words; a box with no text and nothing in it gets its size.
    const words = textBoxes(element);
    for (const rect of words) shapes.push({ kind: "text", rect });
    const hasChildren = Array.from(element.children).some(child => child.getBoundingClientRect().width > 0);
    // Visually hidden helpers (1 × 1, clipped) aren't worth a size.
    if (rect.w >= 4 && rect.h >= 4 && !words.length && (!hasChildren || element instanceof SVGSVGElement || element instanceof HTMLImageElement)) {
      label(`${fmt(rect.w)} × ${fmt(rect.h)}`, rect.x, rect.y + rect.h, "size");
    }

    // The space between neighbouring children, down a column or along a row.
    const kids = Array.from(element.children)
      .map(child => ({ child, box: child.getBoundingClientRect() }))
      .filter(({ child, box }) => box.width >= 1 && box.height >= 1 && getComputedStyle(child).position !== "absolute" && getComputedStyle(child).position !== "fixed");
    for (let index = 1; index < kids.length; index += 1) {
      const a = kids[index - 1]!.box;
      const b = kids[index]!.box;
      if (b.top >= a.bottom - 0.5 && b.top - a.bottom >= 1) {
        const left = Math.min(a.left, b.left);
        const gap = { x: left, y: a.bottom, w: Math.max(a.right, b.right) - left, h: b.top - a.bottom };
        if (!touches(gap, area)) continue;
        shapes.push({ kind: "gap", rect: gap });
        label(fmt(gap.h), gap.x + gap.w, gap.y + gap.h / 2, "gap");
      } else if (b.left >= a.right - 0.5 && b.left - a.right >= 1) {
        const top = Math.min(a.top, b.top);
        const gap = { x: a.right, y: top, w: b.left - a.right, h: Math.max(a.bottom, b.bottom) - top };
        if (!touches(gap, area)) continue;
        shapes.push({ kind: "gap", rect: gap });
        label(fmt(gap.w), gap.x + gap.w / 2, gap.y + gap.h / 2, "hgap");
      }
    }
  }
  return { shapes, labels };
}

const place = (rect: Rect): CSSProperties => ({ transform: `translate(${rect.x}px, ${rect.y}px)`, width: rect.w, height: rect.h });

export function SpacingInspector({ onExit }: { onExit: () => void }) {
  const [area, setArea] = useState<Rect | null>(null);
  const [dragging, setDragging] = useState(false);
  const [drawing, setDrawing] = useState<Drawing | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const areaRef = useRef<Rect | null>(null);
  const root = useRef<HTMLDivElement>(null);

  // Labels that land on one another are moved down, one under the next, so every value can be read.
  useLayoutEffect(() => {
    const placed: DOMRect[] = [];
    for (const item of root.current?.querySelectorAll<HTMLElement>("[data-label]") ?? []) {
      item.style.marginTop = "";
      let box = item.getBoundingClientRect();
      for (let tries = 0; tries < 6; tries += 1) {
        const hit = placed.find(other => box.left < other.right && box.right > other.left && box.top < other.bottom && box.bottom > other.top);
        if (!hit) break;
        item.style.marginTop = `${(parseFloat(item.style.marginTop) || 0) + hit.bottom - box.top + 2}px`;
        box = item.getBoundingClientRect();
      }
      placed.push(box);
    }
  }, [drawing]);

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-spacing-inspect", "");
    let frame = 0;
    const redraw = () => {
      frame = 0;
      setDrawing(areaRef.current ? measure(areaRef.current) : null);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(redraw);
    };
    const exempt = (event: Event) => event.target instanceof Element && !!event.target.closest(EXEMPT);

    const down = (event: PointerEvent) => {
      if (exempt(event) || event.button !== 0) return;
      event.preventDefault();
      event.stopPropagation();
      start.current = { x: event.clientX, y: event.clientY };
      setDragging(true);
    };
    const move = (event: PointerEvent) => {
      const from = start.current;
      if (!from) return;
      const next = { x: Math.min(from.x, event.clientX), y: Math.min(from.y, event.clientY), w: Math.abs(event.clientX - from.x), h: Math.abs(event.clientY - from.y) };
      areaRef.current = next.w >= MIN_DRAG || next.h >= MIN_DRAG ? next : null;
      setArea(areaRef.current);
      schedule();
    };
    const up = (event: PointerEvent) => {
      if (!start.current) return;
      event.preventDefault();
      event.stopPropagation();
      start.current = null;
      setDragging(false);
      if (!areaRef.current) setDrawing(null);
    };
    const swallow = (event: Event) => {
      if (exempt(event)) return;
      event.preventDefault();
      event.stopPropagation();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      onExit();
    };

    // On window, so they run before anything on document: Radix closes a dialog on Esc in document's capture phase.
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("pointermove", move, true);
    window.addEventListener("pointerup", up, true);
    window.addEventListener("click", swallow, true);
    window.addEventListener("dragstart", swallow, true);
    window.addEventListener("selectstart", swallow, true);
    window.addEventListener("scroll", schedule, true);
    window.addEventListener("resize", schedule);
    window.addEventListener("keydown", key, true);
    return () => {
      cancelAnimationFrame(frame);
      root.removeAttribute("data-spacing-inspect");
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("pointermove", move, true);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("click", swallow, true);
      window.removeEventListener("dragstart", swallow, true);
      window.removeEventListener("selectstart", swallow, true);
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("resize", schedule);
      window.removeEventListener("keydown", key, true);
    };
  }, [onExit]);

  return createPortal(
    <div ref={root} className={styles.root} data-spacing-inspector="" aria-hidden="true">
      {area ? (
        <>
          <div className={styles.marquee} style={place(area)} data-dragging={dragging || undefined} />
          {drawing ? (
            <div className={styles.clip} style={place(area)}>
              <div className={styles.shift} style={{ transform: `translate(${-area.x}px, ${-area.y}px)` }}>
                {drawing.shapes.map((shape, index) => <div key={index} className={styles[shape.kind]} style={place(shape.rect)} />)}
              </div>
            </div>
          ) : null}
          {drawing?.labels.map((item, index) => (
            <span key={index} className={styles.label} data-label="" data-kind={item.kind} style={{ transform: `translate(${item.x}px, ${item.y}px)` }}>{item.text}</span>
          ))}
          <span className={styles.label} data-kind="area" style={{ transform: `translate(${area.x + area.w}px, ${area.y + area.h}px)` }}>{fmt(area.w)} × {fmt(area.h)}</span>
        </>
      ) : null}
    </div>,
    document.body,
  );
}
