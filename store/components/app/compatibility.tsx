/**
 * An app page's Compatibility (Anil, 2026-11-18), after What's new: will it run on my machine?
 * - Its title (h2) and a line saying the CLI picks the right build by itself.
 * - A row per system, macOS, Linux and Windows, between hairlines: the system's mark and name, then every processor
 *   the store builds for on that system, as chips: one the app has a package for is filled with a check; one it hasn't
 *   is faint with a dashed edge, so what's missing reads at a glance (the catalog's apps build 1 to 9 of the 9).
 * - Under a row, the developer's public reason for each required target the current release leaves out
 *   (unsupported_targets: Linux x86-64, Windows x64, macOS Apple silicon), where there is one.
 * Server-rendered.
 */
import { Check } from "lucide-react";
import { OsMark } from "@/components/home/os-icons";
import { osOf, targetLabel, TARGETS } from "@/lib/format";
import type { App } from "@/lib/types";
import styles from "./compatibility.module.css";

const SYSTEMS = ["macOS", "Linux", "Windows"];

export function Compatibility({ app }: { app: App }) {
  const built = new Set(app.targets);
  const reasons = app.unsupported_targets ?? {};
  return (
    <section className={styles.section} aria-labelledby="compatibility-title">
      <h2 id="compatibility-title" className={styles.title}>Compatibility</h2>
      <p className={styles.lede}>The CLI installs the build for your system and processor by itself.</p>
      <ul className={styles.systems}>
        {SYSTEMS.map(system => {
          const targets = TARGETS.filter(target => osOf(target) === system);
          const missing = targets.filter(target => !built.has(target) && reasons[target]);
          const any = targets.some(target => built.has(target));
          return (
            <li key={system} className={styles.system}>
              <p className={styles.name} data-none={any ? undefined : ""}>
                <OsMark system={system} className={styles.mark} />
                {system}
              </p>
              <ul className={styles.chips} aria-label={`${system} processors`}>
                {targets.map(target => {
                  const label = targetLabel(target).replace(`${system} `, "");
                  return built.has(target) ? (
                    <li key={target} className={styles.chip}><Check size={14} strokeWidth={2.25} aria-hidden="true" className={styles.check} />{label}</li>
                  ) : (
                    <li key={target} className={styles.chip} data-off=""><span className="sr-only">Not available: </span>{label}</li>
                  );
                })}
              </ul>
              {missing.map(target => (
                <p key={target} className={styles.reason}><strong>No {targetLabel(target)} build.</strong> {reasons[target]}</p>
              ))}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
