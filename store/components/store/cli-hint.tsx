"use client";
/**
 * "silicon-apps" inside an install command, underlined: hovering, focusing or tapping it opens Arc's hover card with
 * the one command that installs silicon-apps itself, for the visitor's own system (detected in the browser), and a
 * toggle to see the other one. Without script it is a plain link to the home page's "Get the CLI" section.
 */
import { useState, useSyncExternalStore, type MouseEvent } from "react";
import { ArrowUpRight } from "lucide-react";
import { HoverCard } from "@/components/silicon-ui/hover-card/hover-card";
import SegmentedControl from "@/components/silicon-ui/segmented-control/segmented-control";
import { CopyCode } from "@/components/site/copy-code";
import { INSTALL_UNIX, INSTALL_WINDOWS, LINKS } from "@/lib/site";
import styles from "./cli-hint.module.css";

type System = "unix" | "windows";

/** The visitor's system, best effort: Windows, or everything else (macOS and Linux share one installer). */
export function detectSystem(): System {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = nav.userAgentData?.platform || nav.platform || nav.userAgent;
  return /win/i.test(platform) ? "windows" : "unix";
}

const noop = () => () => {};

function InstallCard() {
  // The visitor's own system first; the toggle overrides it.
  const detected = useSyncExternalStore(noop, detectSystem, () => "unix" as System);
  const [chosen, setSystem] = useState<System | null>(null);
  const system = chosen ?? detected;
  const command = system === "windows" ? INSTALL_WINDOWS : INSTALL_UNIX;
  return (
    <div className={styles.card}>
      <p className={styles.title}>Get silicon-apps first</p>
      <p className={styles.text}>One install, and every app is a command away. It keeps itself and your apps up to date.</p>
      <SegmentedControl
        label="System"
        value={system}
        onValueChange={value => setSystem(value as System)}
        options={[{ value: "unix", label: "macOS & Linux" }, { value: "windows", label: "Windows" }]}
      />
      <div className={styles.command}>
        <pre className={styles.code}><code>{command}</code></pre>
        <CopyCode label="Copy the silicon-apps installer" value={command} />
      </div>
      <a className={styles.guide} href={LINKS.installDocs} target="_blank" rel="noopener noreferrer">Install guide<ArrowUpRight size={13} strokeWidth={1.75} aria-hidden="true" /></a>
    </div>
  );
}

export function CliName({ name = "silicon-apps" }: { name?: string }) {
  return (
    <HoverCard content={<InstallCard />} side="bottom" align="start" className={styles.surface}>
      <a href="/#get-the-cli" className={styles.trigger} onClick={(event: MouseEvent) => event.preventDefault()}>{name}</a>
    </HoverCard>
  );
}
