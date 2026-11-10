"use client";

import { useEffect } from "react";
import { usePathname, useRouter } from "next/navigation";

// One receiver per visible, signed-in tab. Apps owns the Ting connection and
// rechecks access; the browser keeps only the durable Apps event cursor.
const TYPES = ["app.details_changed", "app.access_changed", "app.published", "release.created", "release.promoted", "release.withdrawn", "author.joined", "author.removed", "review.updated", "review.removed"];

export function LiveEvents({ accountId }: { accountId: string }) {
  const router = useRouter();
  const pathname = usePathname();
  useEffect(() => {
    let source: EventSource | null = null;
    let reconnect: ReturnType<typeof setTimeout> | undefined;
    let refresh: ReturnType<typeof setTimeout> | undefined;
    let lastId = "";
    let failures = 0;
    let dirty = false;
    let stopped = false;
    const reconcile = () => {
      if (dirty || stopped || document.hidden || refresh) return;
      refresh = setTimeout(() => {
        refresh = undefined;
        if (!dirty && !stopped && !document.hidden) router.refresh();
      }, 500);
    };
    const connect = () => {
      if (stopped || document.hidden || source) return;
      const query = new URLSearchParams({ transport: "auto", types: TYPES.join(",") });
      if (lastId) query.set("last_event_id", lastId);
      const connection = new EventSource(`/v1/events/stream?${query}`);
      source = connection;
      const started = Date.now();
      // Reconcile after registration, including reconnects with no events yet.
      connection.onopen = reconcile;
      for (const type of TYPES) connection.addEventListener(type, (event) => {
        lastId = (event as MessageEvent).lastEventId || lastId;
        reconcile();
      });
      connection.onerror = () => {
        connection.close();
        if (source === connection) source = null;
        if (stopped || document.hidden) return;
        if (Date.now() - started >= 60_000) failures = 0;
        const delay = Math.min(30_000, 1000 * 2 ** Math.min(failures++, 5) + Math.random() * 250);
        reconnect = setTimeout(connect, delay);
      };
    };
    const visibility = () => {
      clearTimeout(reconnect);
      if (document.hidden) { source?.close(); source = null; }
      else connect();
    };
    // A server refresh must never replace an unfinished review or settings edit.
    const editing = (event: Event) => {
      if (event.target instanceof HTMLElement && event.target.closest("form")) dirty = true;
    };
    connect();
    document.addEventListener("visibilitychange", visibility);
    document.addEventListener("input", editing);
    document.addEventListener("change", editing);
    return () => {
      stopped = true;
      source?.close();
      clearTimeout(reconnect);
      clearTimeout(refresh);
      document.removeEventListener("visibilitychange", visibility);
      document.removeEventListener("input", editing);
      document.removeEventListener("change", editing);
    };
  }, [accountId, pathname, router]);
  return null;
}
