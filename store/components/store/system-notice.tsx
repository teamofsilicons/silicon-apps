"use client";
/**
 * On an app's page, a quiet warning when the app has no package for the visitor's own system (detected in the
 * browser), before they try the install command: "Not available for macOS yet. It runs on Linux." Nothing is shown
 * when it is available, when the system is unknown, or without script.
 */
import { useSyncExternalStore } from "react";
import { CircleAlert } from "lucide-react";
import styles from "./system-notice.module.css";

function visitorSystem(): string | null {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string; mobile?: boolean } };
  const platform = `${nav.userAgentData?.platform ?? ""} ${nav.platform ?? ""} ${nav.userAgent}`;
  if (/android|iphone|ipad|ipod/i.test(platform) || nav.userAgentData?.mobile) return "phone";
  if (/mac/i.test(platform)) return "macOS";
  if (/win/i.test(platform)) return "Windows";
  if (/linux|x11|cros/i.test(platform)) return "Linux";
  return null;
}

const noop = () => () => {};

/** `systems` are the operating systems the app has packages for, by name (macOS, Linux, Windows). */
export function SystemNotice({ systems }: { systems: string[] }) {
  // The server (and the first hydration pass) knows no system; the browser reads it once.
  const system = useSyncExternalStore(noop, visitorSystem, () => null);
  if (!system || !systems.length) return null;
  if (system === "phone") {
    return <p className={styles.notice}><CircleAlert size={15} strokeWidth={1.75} aria-hidden="true" />Install it from a computer: it runs on {systems.join(", ")}.</p>;
  }
  if (systems.includes(system)) return null;
  return <p className={styles.notice} role="note"><CircleAlert size={15} strokeWidth={1.75} aria-hidden="true" />Not available for {system} yet. It runs on {systems.join(", ")}.</p>;
}
