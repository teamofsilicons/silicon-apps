/**
 * The store server's only way to the Apps API. Pages and server actions call it with the visitor's own session: the
 * browser's HttpOnly `apps_session` cookie (set by the API's /v1/auth/callback on this same origin) is forwarded and
 * nothing else. Browser JavaScript never sees a token.
 *
 * Mutations carry the store's public Origin (the API checks it against APPS_ALLOWED_ORIGINS whenever a session cookie
 * is present) and an Idempotency-Key. Server actions are only reached after Next has checked that the form was posted
 * from this origin, so forwarding the Origin is not a way around the API's CSRF check.
 */
import { cookies, headers } from "next/headers";
import { appsApiUrl, siteOrigin } from "./config";
import type { ApiErrorBody } from "./types";

export const SESSION_COOKIE = "apps_session";
export const TELEMETRY_COOKIE = "apps_telemetry";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly hint?: string;
  constructor(status: number, body: ApiErrorBody | null, fallback?: string) {
    super(body?.error?.message || fallback || `The Apps service answered HTTP ${status}.`);
    this.status = status;
    this.code = body?.error?.code || (status === 0 ? "api_unavailable" : "request_failed");
    this.hint = body?.error?.hint;
  }
}

type Options = {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  body?: unknown;
  idempotencyKey?: string;
  /** Leave the visitor's session out (public reads, sitemap). */
  anonymous?: boolean;
  timeoutMs?: number;
};

async function visitorHeaders(anonymous: boolean): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  try {
    const jar = await cookies();
    if (jar.get(TELEMETRY_COOKIE)?.value === "off") out["X-Apps-Telemetry"] = "off";
    if (!anonymous) {
      const session = jar.get(SESSION_COOKIE)?.value;
      if (session && /^[A-Za-z0-9_-]{8,200}$/.test(session)) out.Cookie = `${SESSION_COOKIE}=${session}`;
    }
    const incoming = await headers();
    // Caddy replaces X-Forwarded-For with the real peer address; pass it on so the API sees the visitor, not us.
    const forwarded = incoming.get("x-forwarded-for")?.split(",")[0]?.trim();
    if (forwarded && /^[0-9a-fA-F:.]{2,45}$/.test(forwarded)) out["X-Forwarded-For"] = forwarded;
  } catch {
    // Outside a request (build time): no visitor to forward.
  }
  return out;
}

export async function api<T>(path: string, options: Options = {}): Promise<T> {
  const method = options.method || "GET";
  const headersOut: Record<string, string> = {
    Accept: "application/json",
    "User-Agent": "silicon-apps-store",
    ...(await visitorHeaders(Boolean(options.anonymous))),
  };
  let body: string | undefined;
  if (method !== "GET") {
    headersOut.Origin = siteOrigin();
    headersOut["Idempotency-Key"] = options.idempotencyKey || crypto.randomUUID();
    headersOut["Content-Type"] = "application/json";
    body = JSON.stringify(options.body ?? {});
  }
  let response: Response;
  try {
    response = await fetch(`${appsApiUrl()}${path}`, {
      method,
      headers: headersOut,
      body,
      cache: "no-store",
      redirect: "manual",
      signal: AbortSignal.timeout(options.timeoutMs ?? 10_000),
    });
  } catch {
    throw new ApiError(0, null, "We could not reach the Apps service. Try again in a moment.");
  }
  const text = await response.text();
  let payload: unknown = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    payload = null;
  }
  if (!response.ok) throw new ApiError(response.status, payload as ApiErrorBody | null);
  return payload as T;
}

/** A read that answers null on 404 instead of throwing (missing or not visible to this visitor). */
export async function apiOrNull<T>(path: string, options: Options = {}): Promise<T | null> {
  try {
    return await api<T>(path, options);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) return null;
    throw error;
  }
}

/** Loose on purpose: the API owns the real rule and answers 404 for anything else. */
export const APP_ID_PATTERN = /^[A-Za-z0-9_.-]{1,100}$/;
