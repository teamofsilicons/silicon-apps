"use client";

/**
 * Lets the dev tools work while a modal dialog is open (Anil, 2026-11-10: the dock, the type lab and the text editor
 * were dead over "Add a phone number"). A Radix modal dialog listens on `document` for pointerdown, focusin and
 * focusout (a press on anything outside it closes it, and focus that leaves it is pulled back) and for wheel and touch
 * events (scrolling anywhere outside it is cancelled). Those listeners are
 * added when the dialog opens; this one is added when the dev tools mount, so it runs first on `document` (after
 * React's own root listener, which has already handled the event) and stops the dialog's for events on a dev surface
 * ([data-type-inspect-exempt]). The surfaces also take `pointer-events: auto` and sit above the dialog's layer in CSS.
 * Dev-only: the dev tools never render in production.
 */
import { useEffect } from "react";

const DEV_SURFACE = "[data-type-inspect-exempt]";

const onDevSurface = (node: EventTarget | null) => node instanceof Element && node.closest(DEV_SURFACE) !== null;

export function useDialogPassThrough() {
  useEffect(() => {
    const stop = (event: Event) => {
      const subject = event.type === "focusout" ? (event as FocusEvent).relatedTarget : event.target;
      if (onDevSurface(subject)) event.stopImmediatePropagation();
    };
    // The dialog's scroll lock (react-remove-scroll) cancels wheel and touch scrolling outside it the same way.
    const events = ["pointerdown", "focusin", "focusout", "wheel", "touchstart", "touchmove"] as const;
    for (const name of events) document.addEventListener(name, stop);
    return () => { for (const name of events) document.removeEventListener(name, stop); };
  }, []);
}
