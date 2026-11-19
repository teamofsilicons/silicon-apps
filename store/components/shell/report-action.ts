"use server";
/**
 * Report a problem (the sidebar's foot, components/shell/report.tsx): the same report as `silicon-apps report`,
 * delivered to the maintainers by the API (POST /v1/reports), with the screenshot where there is one (`screenshot`:
 * {name, content_type, data (base64)}; the API takes it from 2026-11-19, see API_CONTRACT.md). The page it was sent from and an idempotency key are bound
 * to the action. A plain form post, so it works without script; it comes back to that page with ?report=sent or what
 * went wrong, which the shell shows as a notice at the top of the page.
 */
import { redirect } from "next/navigation";
import { api, ApiError } from "@/lib/api";
import { localPath } from "@/lib/forms";

/** A screenshot (report-screenshot.tsx): PNG, JPEG or WebP, 5 MB at most, sent to the API as base64 with its type. */
const SCREENSHOT_TYPES = ["image/png", "image/jpeg", "image/webp"];
const SCREENSHOT_MAX = 5 * 1024 * 1024;

/** The page it came from, with ?report= set (any earlier one replaced). */
function back(rawPath: string, outcome: string): string {
  const url = new URL(localPath(rawPath, "/"), "http://store.local");
  url.searchParams.set("report", outcome);
  return `${url.pathname}${url.search}`;
}

export async function sendReport(rawPath: string, rawKey: string, formData: FormData): Promise<void> {
  const message = String(formData.get("message") ?? "").trim();
  const pr = String(formData.get("pr") ?? "").trim();
  if (message.length < 3) redirect(back(rawPath, "empty"));
  if (message.length > 10_000) redirect(back(rawPath, "long"));
  if (pr && !/^https:\/\/\S{3,500}$/.test(pr)) redirect(back(rawPath, "pr"));
  const file = formData.get("screenshot");
  let screenshot: { name: string; content_type: string; data: string } | undefined;
  if (file instanceof File && file.size > 0) {
    if (!SCREENSHOT_TYPES.includes(file.type) || file.size > SCREENSHOT_MAX) redirect(back(rawPath, "image"));
    screenshot = { name: file.name.slice(0, 120) || "screenshot", content_type: file.type, data: Buffer.from(await file.arrayBuffer()).toString("base64") };
  }
  let outcome = "sent";
  try {
    await api("/v1/reports", { method: "POST", body: { message, ...(pr ? { pr } : {}), ...(screenshot ? { screenshot } : {}) }, idempotencyKey: /^[\x21-\x7e]{8,200}$/.test(rawKey) ? rawKey : undefined });
  } catch (error) {
    outcome = error instanceof ApiError && error.status === 503 ? "unavailable" : "failed";
  }
  redirect(back(rawPath, outcome));
}
