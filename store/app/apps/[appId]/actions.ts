"use server";
/**
 * Review writes, as server actions behind plain forms: they work without JavaScript (the browser posts the form and
 * follows the redirect) and Next checks the form came from this origin before either runs. The visitor's own
 * session cookie goes to the API, which checks the account, the app's visibility, the 1 to 5 rating and the
 * 600-character limit again. The hidden idempotency key makes a double submit save once.
 */
import { redirect } from "next/navigation";
import { api, ApiError, APP_ID_PATTERN } from "@/lib/api";

const KEY = /^[\x21-\x7e]{8,200}$/;

function back(appId: string, status: string, code?: string): never {
  const params = new URLSearchParams({ review: status });
  if (code) params.set("code", code);
  redirect(`/apps/${encodeURIComponent(appId)}?${params}#reviews`);
}

function read(formData: FormData) {
  const appId = String(formData.get("app_id") ?? "");
  if (!APP_ID_PATTERN.test(appId)) redirect("/search");
  const key = String(formData.get("idempotency_key") ?? "");
  return { appId, key: KEY.test(key) ? key : crypto.randomUUID() };
}

function codeOf(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.status === 401) return "sign_in";
    if (error.status === 0) return "unavailable";
    return error.code.replace(/[^a-z0-9_]/g, "").slice(0, 60) || "failed";
  }
  return "failed";
}

export async function saveReview(formData: FormData): Promise<void> {
  const { appId, key } = read(formData);
  const rating = Number(formData.get("rating"));
  const text = String(formData.get("text") ?? "").replace(/\r\n/g, "\n").trim();
  if (!Number.isInteger(rating) || rating < 1 || rating > 5) back(appId, "error", "rating");
  if ([...text].length > 600) back(appId, "error", "too_long");
  let failure: string | null = null;
  try {
    await api(`/v1/apps/${encodeURIComponent(appId)}/review`, { method: "PUT", body: { rating, text }, idempotencyKey: key });
  } catch (error) {
    failure = codeOf(error);
  }
  if (failure) back(appId, "error", failure);
  back(appId, "saved");
}

export async function removeReview(formData: FormData): Promise<void> {
  const { appId, key } = read(formData);
  let failure: string | null = null;
  try {
    await api(`/v1/apps/${encodeURIComponent(appId)}/review`, { method: "DELETE", idempotencyKey: key });
  } catch (error) {
    failure = codeOf(error);
  }
  if (failure) back(appId, "error", failure);
  back(appId, "removed");
}
