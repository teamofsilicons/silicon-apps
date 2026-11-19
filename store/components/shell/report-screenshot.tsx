"use client";
/**
 * A screenshot for Report a problem (Anil, 2026-11-19: "a way user can report with a screenshot … ask them if they'd
 * like to take a screenshot from the current screen"). In the report dialog, under what happened:
 * - "Add a screenshot of this page?" with Take Screenshot: the dialog steps aside, the browser asks which tab or screen
 *   to share (this tab first, where the browser can say so), one frame is kept and the sharing stops at once; then the
 *   dialog comes back with the picture. Where the browser can't share a screen (phones), only Attach Image shows.
 * - Attach Image: a picture from the device instead.
 * - Once there is one, a preview with Remove (and Retake).
 * The picture rides in the form as an ordinary file field (`screenshot`), so the form posts it the same way with or
 * without script; without script the field itself shows, a plain file picker. PNG, JPEG or WebP, 5 MB at most (a large
 * capture is scaled to 2560px wide and, if still too big, saved as JPEG).
 */
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { Camera, ImagePlus, X } from "lucide-react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import styles from "./report.module.css";

export const SCREENSHOT_MAX = 5 * 1024 * 1024;
const ACCEPT = "image/png,image/jpeg,image/webp";
const MAX_WIDTH = 2560;

type Shot = { url: string; name: string; size: number };

const noop = () => () => {};
const sizeOf = (bytes: number) => (bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`);
const frame = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));

/** One frame of what the visitor chose to share, as a PNG (a JPEG if the PNG is over the limit). */
async function capture(): Promise<File> {
  const options = { video: { displaySurface: "browser" }, audio: false, preferCurrentTab: true, selfBrowserSurface: "include", surfaceSwitching: "exclude" } as DisplayMediaStreamOptions;
  const stream = await navigator.mediaDevices.getDisplayMedia(options);
  try {
    const video = document.createElement("video");
    video.srcObject = stream;
    video.muted = true;
    await video.play();
    // Let the browser's "sharing this tab" bar settle before the frame is kept.
    await new Promise(resolve => setTimeout(resolve, 350));
    const scale = Math.min(1, MAX_WIDTH / video.videoWidth);
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(video.videoWidth * scale);
    canvas.height = Math.round(video.videoHeight * scale);
    canvas.getContext("2d")!.drawImage(video, 0, 0, canvas.width, canvas.height);
    const toBlob = (type: string, quality?: number) => new Promise<Blob>((resolve, reject) => canvas.toBlob(blob => (blob ? resolve(blob) : reject(new Error("empty"))), type, quality));
    let blob = await toBlob("image/png");
    if (blob.size > SCREENSHOT_MAX) blob = await toBlob("image/jpeg", 0.85);
    return new File([blob], blob.type === "image/png" ? "screenshot.png" : "screenshot.jpg", { type: blob.type });
  } finally {
    stream.getTracks().forEach(track => track.stop());
  }
}

export function ReportScreenshot({ dialogId }: { dialogId: string }) {
  const input = useRef<HTMLInputElement>(null);
  // False while the server renders (the plain file field), true once script runs in the browser.
  const enhanced = useSyncExternalStore(noop, () => true, () => false);
  const canCapture = useSyncExternalStore(noop, () => typeof navigator.mediaDevices?.getDisplayMedia === "function", () => false);
  const [shot, setShot] = useState<Shot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => () => { if (shot) URL.revokeObjectURL(shot.url); }, [shot]);

  /** Put a file in the form's field (or clear it), and show it. */
  function attach(file: File | null) {
    if (!input.current) return;
    const files = new DataTransfer();
    if (file) files.items.add(file);
    input.current.files = files.files;
    setShot(file ? { url: URL.createObjectURL(file), name: file.name, size: file.size } : null);
  }

  function check(file: File | undefined): File | null {
    if (!file) return null;
    if (!ACCEPT.split(",").includes(file.type)) { setError("Choose a PNG, JPEG or WebP image."); return null; }
    if (file.size > SCREENSHOT_MAX) { setError("Choose an image of 5 MB or less."); return null; }
    return file;
  }

  async function take() {
    const dialog = document.getElementById(dialogId);
    setError(null);
    setBusy(true);
    dialog?.hidePopover();
    try {
      await frame();
      attach(check(await capture()));
    } catch (reason) {
      // Declining the browser's prompt is a choice, not a failure: say nothing then.
      if (!(reason instanceof DOMException && reason.name === "NotAllowedError")) setError("The screenshot couldn’t be taken. Attach an image instead.");
    } finally {
      dialog?.showPopover();
      setBusy(false);
    }
  }

  return (
    <div className={styles.shot}>
      {/* eslint-disable-next-line no-restricted-syntax -- Silicon UI has no form-posting file field; a native one keeps the report a plain form that works without script (with script it is hidden and filled by the buttons) */}
      <input
        ref={input}
        id={`${dialogId}-screenshot`}
        type="file"
        name="screenshot"
        accept={ACCEPT}
        className={enhanced ? "sr-only" : styles.file}
        tabIndex={enhanced ? -1 : undefined}
        aria-label="Screenshot (optional)"
        onChange={event => { setError(null); attach(check(event.target.files?.[0])); }}
      />
      {enhanced ? <span id={`${dialogId}-screenshot-label`} className={styles.shotLabel}>Screenshot (optional)</span> : null}
      {!enhanced ? <label htmlFor={`${dialogId}-screenshot`} className={styles.shotHint}>Screenshot (optional): PNG, JPEG or WebP, up to 5 MB.</label> : shot ? (
        <div className={styles.preview}>
          {/* eslint-disable-next-line @next/next/no-img-element -- a local preview of the visitor's own picture (a blob: URL), never fetched or resized */}
          <img src={shot.url} alt="The screenshot to send" className={styles.thumb} />
          <div className={styles.previewWords}>
            <span className={styles.previewName}>{shot.name}</span>
            <span className={styles.shotHint}>{sizeOf(shot.size)} · sent with your report</span>
          </div>
          <div className={styles.previewActions}>
            {canCapture ? <Button type="button" variant="ghost" size="sm" onClick={take} disabled={busy}>Retake</Button> : null}
            <Button type="button" variant="ghost" size="sm" onClick={() => attach(null)} aria-label="Remove the screenshot"><X size={16} strokeWidth={2} aria-hidden="true" /></Button>
          </div>
        </div>
      ) : (
        <div className={styles.askActions} role="group" aria-labelledby={`${dialogId}-screenshot-label`}>
          {canCapture ? <Button type="button" variant="secondary" size="sm" onClick={take} disabled={busy}><Camera size={16} strokeWidth={2} aria-hidden="true" />Take Screenshot of This Page</Button> : null}
          <Button type="button" variant={canCapture ? "ghost" : "secondary"} size="sm" onClick={() => input.current?.click()}><ImagePlus size={16} strokeWidth={2} aria-hidden="true" />Attach Image</Button>
        </div>
      )}
      {error ? <p className={styles.shotError} role="alert">{error}</p> : null}
    </div>
  );
}
