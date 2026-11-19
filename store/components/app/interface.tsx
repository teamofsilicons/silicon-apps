/**
 * An app page's Build an interface (from main's interface metadata, fitted to the redesign on 2026-11-19), after
 * Compatibility, only for an app that publishes any: may I build my own interface on this app, and how?
 * - Its title (h2) and a line saying what an interface is here.
 * - The interface API's documentation, a link out.
 * - Folds between hairlines, as What's new's: each use case with its Markdown files (each shown in mono, with Download
 *   Markdown), then the interface license: a preset's words and what the terms don't grant, a link, or the license
 *   file itself.
 * Folds are details, so no script. Server-rendered.
 */
import type { ReactNode } from "react";
import { ArrowUpRight, Download, Triangle } from "lucide-react";
import { safeUrl } from "@/lib/format";
import type { App } from "@/lib/types";
import styles from "./interface.module.css";

const LICENSES: Record<string, string> = {
  free: "You may use this app’s interface API to build, distribute and operate interfaces, including commercially, without payment or attribution.",
  attribution: "You may use this app’s interface API to build, distribute and operate interfaces, including commercially. Credit the app by name and link to its app page wherever you give interface credits.",
  attribution_unless_paid: "You may use this app’s interface API to build, distribute and operate interfaces, including commercially. Credit the app by name and link to its app page wherever you give interface credits. Attribution is not required if the app publisher has granted you a paid attribution waiver. Arrange its price and terms with the publisher.",
  permission_required: "Ask the app publisher for permission before using this app’s interface API in an interface.",
};
const LICENSE_SCOPE = "These terms do not grant rights to the app’s source code, trademarks or user data, or waive other applicable terms.";

function Fold({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <details className={styles.fold}>
      <summary>
        <Triangle className={styles.triangle} size={10} strokeWidth={2} fill="currentColor" aria-hidden="true" />
        {title}
        {count !== undefined ? <span className={styles.count}>{count}</span> : null}
      </summary>
      <div className={styles.foldBody}>{children}</div>
    </details>
  );
}

/** A file to read in place and download (a data: URL, so nothing is fetched). */
function File({ name, content, type }: { name: string; content: string; type: string }) {
  return (
    <div className={styles.file}>
      <div className={styles.fileHead}>
        <span className={styles.fileName}>{name}</span>
        <a href={`data:${type};charset=utf-8,${encodeURIComponent(content)}`} download={name} className={styles.download}><Download size={14} strokeWidth={2} aria-hidden="true" />Download</a>
      </div>
      <pre className={styles.content}>{content}</pre>
    </div>
  );
}

export function Interface({ app }: { app: App }) {
  const docs = safeUrl(app.interface_api_documentation_url);
  const cases = app.interface_use_cases ?? [];
  const license = app.interface_license;
  if (!docs && !cases.length && !license) return null;
  const licenseUrl = license?.kind === "link" ? safeUrl(license.url) : undefined;
  return (
    <section className={styles.section} aria-labelledby="interface-title">
      <h2 id="interface-title" className={styles.title}>Build an interface</h2>
      <p className={styles.lede}>Make your own way to use {app.name}, on its interface API.</p>
      {docs ? <p className={styles.docs}><a href={docs} target="_blank" rel="nofollow ugc noopener noreferrer">Interface API documentation<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /></a></p> : null}
      {cases.length || license ? (
        <div className={styles.folds}>
          {cases.map((useCase, index) => (
            <Fold key={`${useCase.name}-${index}`} title={useCase.name} count={useCase.files.length}>
              {useCase.files.map((file, fileIndex) => <File key={`${file.name}-${fileIndex}`} name={file.name} content={file.content} type="text/markdown" />)}
            </Fold>
          ))}
          {license ? (
            <Fold title="Interface license">
              {license.kind === "preset" ? <><p className={styles.text}>{LICENSES[license.preset]}</p><p className={styles.scope}>{LICENSE_SCOPE}</p></> : null}
              {licenseUrl ? <p className={styles.docs}><a href={licenseUrl} target="_blank" rel="nofollow ugc noopener noreferrer">Read the interface license<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /></a></p> : null}
              {license.kind === "file" ? <File name={license.name} content={license.content} type="text/plain" /> : null}
            </Fold>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
