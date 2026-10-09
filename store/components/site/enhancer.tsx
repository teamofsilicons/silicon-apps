"use client";

/**
 * The store's one behaviour island, as on the developer site (developer/components/site/enhancer.tsx). The pages are
 * server-rendered HTML that works without script; this adds the small things that need it, by delegation over the
 * markup, so no part of the page has to hydrate:
 *
 * - copy buttons ([data-copy]): copy the block's code (or data-copy-value) and confirm for a moment;
 * - carousel buttons ([data-carousel-step] on a button that controls a scroll-snap track): scroll by one screen, and
 *   disable themselves at either end;
 * - character counters ([data-count-for]): how many of a field's characters are left;
 * - the menu (#site-menu): close it when one of its links only moves within this page.
 *
 * It renders one visually hidden status line, so a copy is announced to screen readers.
 */
import { useEffect, useState } from "react";

const COPIED_MS = 1600;

function copyText(button: HTMLElement): string {
  const value = button.getAttribute("data-copy-value");
  if (value !== null) return value;
  return button.closest("[data-docs-code]")?.querySelector("pre")?.textContent ?? "";
}

async function writeClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) return navigator.clipboard.writeText(text);
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  document.execCommand("copy");
  area.remove();
}

/** Carousel buttons: enabled only where the track can still move that way. */
function useCarousels() {
  useEffect(() => {
    const tracks = [...new Set([...document.querySelectorAll<HTMLElement>("[data-carousel-step]")].map(button => button.getAttribute("aria-controls")).filter(Boolean) as string[])]
      .map(id => document.getElementById(id))
      .filter((track): track is HTMLElement => track !== null);
    const update = (track: HTMLElement) => {
      const max = track.scrollWidth - track.clientWidth;
      for (const button of document.querySelectorAll<HTMLButtonElement>(`[data-carousel-step][aria-controls="${track.id}"]`)) {
        const step = Number(button.getAttribute("data-carousel-step"));
        button.disabled = max <= 4 || (step < 0 ? track.scrollLeft <= 4 : track.scrollLeft >= max - 4);
      }
      track.closest("[data-carousel]")?.toggleAttribute("data-scrollable", max > 4);
    };
    const cleanups = tracks.map(track => {
      const onChange = () => update(track);
      onChange();
      track.addEventListener("scroll", onChange, { passive: true });
      window.addEventListener("resize", onChange);
      return () => {
        track.removeEventListener("scroll", onChange);
        window.removeEventListener("resize", onChange);
      };
    });
    return () => cleanups.forEach(cleanup => cleanup());
  }, []);
}

/** "123 characters left" under a field with a maxlength. */
function useCounters() {
  useEffect(() => {
    const counters = [...document.querySelectorAll<HTMLElement>("[data-count-for]")];
    const cleanups = counters.map(counter => {
      const field = document.getElementById(counter.getAttribute("data-count-for") ?? "") as HTMLTextAreaElement | HTMLInputElement | null;
      if (!field || field.maxLength <= 0) return () => {};
      const update = () => {
        const left = field.maxLength - [...field.value].length;
        counter.textContent = `${left} ${left === 1 ? "character" : "characters"} left`;
        counter.toggleAttribute("data-low", left <= 40);
      };
      update();
      field.addEventListener("input", update);
      return () => field.removeEventListener("input", update);
    });
    return () => cleanups.forEach(cleanup => cleanup());
  }, []);
}

export function Enhancer() {
  const [status, setStatus] = useState("");
  useCarousels();
  useCounters();

  useEffect(() => {
    const timers = new Map<HTMLElement, number>();
    const onClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;

      const copy = target.closest<HTMLElement>("[data-copy]");
      if (copy) {
        const label = copy.getAttribute("data-label") ?? "Copy";
        writeClipboard(copyText(copy)).then(
          () => {
            copy.setAttribute("data-state", "copied");
            copy.setAttribute("aria-label", "Copied");
            setStatus("Copied to the clipboard");
          },
          () => setStatus("Could not copy: the browser refused access to the clipboard. Select the command and copy it yourself."),
        );
        window.clearTimeout(timers.get(copy));
        timers.set(copy, window.setTimeout(() => {
          copy.setAttribute("data-state", "idle");
          copy.setAttribute("aria-label", label);
          setStatus("");
        }, COPIED_MS));
        return;
      }

      const step = target.closest<HTMLButtonElement>("[data-carousel-step]");
      if (step) {
        const track = document.getElementById(step.getAttribute("aria-controls") ?? "");
        if (!track) return;
        const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        track.scrollBy({ left: Number(step.getAttribute("data-carousel-step")) * track.clientWidth * 0.85, behavior: reduced ? "auto" : "smooth" });
        return;
      }

      // A menu link that only moves within this page leaves the page under the open menu: close it.
      const link = target.closest<HTMLAnchorElement>("#site-menu a[href]");
      if (link && link.pathname === window.location.pathname && link.hash) {
        const menu = document.getElementById("site-menu") as (HTMLElement & { hidePopover?: () => void }) | null;
        menu?.hidePopover?.();
      }
    };
    document.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("click", onClick);
      for (const timer of timers.values()) window.clearTimeout(timer);
    };
  }, []);

  return <span className="sr-only" role="status" aria-live="polite">{status}</span>;
}
