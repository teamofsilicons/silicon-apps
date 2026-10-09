/**
 * The visitor's session. The API's browser sign-in (/v1/auth/login, reached through /sign-in) sets the HttpOnly
 * `apps_session` cookie on this origin; the store forwards it to the API with the visitor's own requests and never
 * shows it to page scripts.
 */
import { cache } from "react";
import { cookies } from "next/headers";
import { api, SESSION_COOKIE } from "./api";
import type { Account, Session } from "./types";

/** The signed-in Carbon or Silicon for this request, or null. One API call per request at most. */
export const getAccount = cache(async (): Promise<Account | null> => {
  const jar = await cookies();
  if (!jar.get(SESSION_COOKIE)?.value) return null;
  try {
    const session = await api<Session>("/v1/session", { timeoutMs: 5_000 });
    return session.authenticated ? session.account : null;
  } catch {
    return null;
  }
});

/** Sign in through Silicon Accounts and come back to `returnTo` (a local path) on this store. */
export function signInHref(returnTo = "/"): string {
  const local = returnTo.startsWith("/") && !returnTo.startsWith("//") ? returnTo : "/";
  return `/sign-in?return_to=${encodeURIComponent(local)}`;
}
