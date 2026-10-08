import { useCallback, useEffect, useRef, useState } from "react";
export class ApiError extends Error {
  code: string;
  hint?: string;
  details?: unknown;
  status: number;
  constructor(
    status: number,
    body: {
      error?: {
        code?: string;
        message?: string;
        hint?: string;
        details?: unknown;
      };
    },
  ) {
    super(body.error?.message || `The service returned HTTP ${status}.`);
    this.code = body.error?.code || "request_failed";
    this.hint = body.error?.hint;
    this.details = body.error?.details;
    this.status = status;
  }
}
const BASE = (import.meta.env.VITE_APPS_API_URL || "").replace(/\/$/, "");
const pendingMutations = new Map<string, string>();
export async function api<T>(
  path: string,
  options: {
    method?: string;
    body?: unknown;
    signal?: AbortSignal;
    key?: string;
    contentType?: string;
  } = {},
): Promise<T> {
  const method = options.method || "GET";
  const headers: Record<string, string> = { Accept: "application/json" };
  if (localStorage.getItem("apps.telemetry") === "false")
    headers["X-Apps-Telemetry"] = "off";
  if (method !== "GET")
    headers["Idempotency-Key"] = options.key || crypto.randomUUID();
  let body: BodyInit | undefined;
  if (options.body instanceof Blob) {
    headers["Content-Type"] = options.contentType || "application/gzip";
    body = options.body;
  } else if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(options.body);
  }
  let fingerprint: string | undefined;
  if (method !== "GET" && !options.key) {
    const bytes =
      options.body instanceof Blob
        ? await options.body.arrayBuffer()
        : new TextEncoder().encode(String(body || ""));
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    fingerprint = `${method}:${path}:${Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("")}`;
    headers["Idempotency-Key"] =
      pendingMutations.get(fingerprint) || headers["Idempotency-Key"];
    pendingMutations.set(fingerprint, headers["Idempotency-Key"]);
    if (pendingMutations.size > 256)
      pendingMutations.delete(pendingMutations.keys().next().value!);
  }
  let response: Response;
  try {
    response = await fetch(`${BASE}/v1${path}`, {
      method,
      headers,
      body,
      signal: options.signal,
      credentials: "include",
    });
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") throw error;
    throw new ApiError(0, {
      error: {
        code: "network_unavailable",
        message: "Could not reach Silicon Apps.",
        hint: "Check your connection and try again. Retrying keeps the same request key, so an accepted action is not repeated.",
      },
    });
  }
  const payload = await response.json().catch(() => ({}));
  if (
    fingerprint &&
    (response.ok ||
      (response.status < 500 &&
        response.status !== 408 &&
        response.status !== 429))
  )
    pendingMutations.delete(fingerprint);
  if (!response.ok) throw new ApiError(response.status, payload);
  return payload as T;
}
export const login = () => {
  window.location.assign(
    `${BASE}/v1/auth/login?return_to=${encodeURIComponent(window.location.pathname + window.location.search)}`,
  );
};
export function useResource<T>(path: string | null, revision = 0) {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<Error>();
  const [loading, setLoading] = useState(true);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!path) {
      setLoading(false);
      setData(undefined);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError(undefined);
    api<T>(path, { signal: controller.signal })
      .then(setData)
      .catch((error) => {
        if (error.name !== "AbortError") setError(error);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [path, revision, retry]);
  return {
    data,
    error,
    loading,
    reload: () => setRetry((n) => n + 1),
    setData,
  };
}
export function useMutation(onDone?: () => void) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<Error>();
  const run = async <T>(action: () => Promise<T>): Promise<T | undefined> => {
    setPending(true);
    setError(undefined);
    try {
      const result = await action();
      onDone?.();
      return result;
    } catch (error) {
      setError(error instanceof Error ? error : new Error(String(error)));
      return undefined;
    } finally {
      setPending(false);
    }
  };
  return { pending, error, run, clearError: () => setError(undefined) };
}
const pendingSaves = new Map<symbol, () => Promise<void>>();
export async function flushPendingSaves() {
  await Promise.all([...pendingSaves.values()].map((save) => save()));
}
export function useAutosave<T extends object>(
  path: string,
  initial: T,
  method = "PATCH",
  onSaved?: (result: unknown) => void,
) {
  const [draft, setDraft] = useState(initial);
  const [status, setStatus] = useState<
    "saved" | "saving" | "unsaved" | "error"
  >("saved");
  const [error, setError] = useState<Error>();
  const latest = useRef(initial);
  const dirty = useRef(false);
  const version = useRef(0);
  const running = useRef<Promise<void> | null>(null);
  const callback = useRef(onSaved);
  callback.current = onSaved;
  const save = useCallback(async (): Promise<void> => {
    if (running.current) {
      await running.current;
      if (dirty.current) return save();
      return;
    }
    if (!dirty.current) return;
    const current = version.current;
    setStatus("saving");
    const request = api(path, { method, body: latest.current })
      .then((result) => {
        if (version.current === current) {
          dirty.current = false;
          setStatus("saved");
          setError(undefined);
          callback.current?.(result);
        }
      })
      .catch((error) => {
        setStatus("error");
        setError(error);
        throw error;
      })
      .finally(() => {
        running.current = null;
      });
    running.current = request;
    await request;
  }, [path, method]);
  const update = useCallback((next: Partial<T>) => {
    dirty.current = true;
    version.current++;
    setStatus("unsaved");
    setDraft((value) => {
      const nextDraft = { ...value, ...next };
      latest.current = nextDraft;
      return nextDraft;
    });
  }, []);
  useEffect(() => {
    const key = Symbol(path);
    pendingSaves.set(key, save);
    return () => {
      pendingSaves.delete(key);
      void save().catch(() => {});
    };
  }, [path, save]);
  useEffect(() => {
    if (!dirty.current) return;
    const timer = window.setTimeout(() => {
      void save().catch(() => {});
    }, 800);
    return () => window.clearTimeout(timer);
  }, [draft, save]);
  useEffect(() => {
    const guard = (event: BeforeUnloadEvent) => {
      if (dirty.current) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, []);
  return {
    draft,
    update,
    status,
    error,
    retry: () => {
      void save().catch(() => {});
    },
    save,
  };
}
