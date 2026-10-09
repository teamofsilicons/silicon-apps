/**
 * Rate limits for the store's own public endpoint (the MCP server), per client address and per bucket, in fixed
 * one-minute windows held in this process's memory (the store runs as one instance), as on the developer site
 * (developer/lib/server/rate-limit.ts). Every answer carries RateLimit-Limit, RateLimit-Remaining and RateLimit-Reset;
 * one over the limit is 429 with Retry-After and a structured error. The limits are in lib/site.ts and documented on
 * the home page.
 *
 * The client address is the first X-Forwarded-For entry: Caddy, in front of the store, sets it from the connection
 * (header_up X-Forwarded-For {remote_host}, trusting no proxy before it), so a client cannot choose it.
 */

export interface Limit {
  limit: number;
  windowSeconds: number;
}

export interface RateDecision {
  ok: boolean;
  limit: number;
  remaining: number;
  /** Seconds until the window resets. */
  reset: number;
  headers: Record<string, string>;
}

const windows = new Map<string, { count: number; resetsAt: number }>();
let lastSweep = 0;

export function clientAddress(request: Request): string {
  const forwarded = request.headers.get("x-forwarded-for")?.split(",")[0]?.trim();
  return forwarded || request.headers.get("x-real-ip")?.trim() || "direct";
}

function sweep(now: number) {
  if (now - lastSweep < 60_000 && windows.size < 50_000) return;
  lastSweep = now;
  for (const [key, entry] of windows) if (entry.resetsAt <= now) windows.delete(key);
}

export function rateLimit(request: Request, bucket: string, { limit, windowSeconds }: Limit): RateDecision {
  const now = Date.now();
  sweep(now);
  const key = `${bucket}:${clientAddress(request)}`;
  let entry = windows.get(key);
  if (!entry || entry.resetsAt <= now) {
    entry = { count: 0, resetsAt: now + windowSeconds * 1000 };
    windows.set(key, entry);
  }
  entry.count++;
  const reset = Math.max(1, Math.ceil((entry.resetsAt - now) / 1000));
  const remaining = Math.max(0, limit - entry.count);
  const ok = entry.count <= limit;
  const headers: Record<string, string> = {
    "RateLimit-Limit": String(limit),
    "RateLimit-Remaining": String(remaining),
    "RateLimit-Reset": String(reset),
    "RateLimit-Policy": `${limit};w=${windowSeconds}`,
  };
  if (!ok) headers["Retry-After"] = String(reset);
  return { ok, limit, remaining, reset, headers };
}

/** The structured error body of a refused request. */
export function rateLimitedError(decision: RateDecision, windowSeconds: number) {
  return {
    code: "rate_limited",
    message: `Too many requests: at most ${decision.limit} every ${windowSeconds} seconds from one address.`,
    hint: `Wait ${decision.reset} second${decision.reset === 1 ? "" : "s"} (Retry-After), then try again.`,
  };
}

/** Tests only: forget every window. */
export function resetRateLimits(): void {
  windows.clear();
}
