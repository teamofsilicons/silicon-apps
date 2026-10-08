import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  AlertCircle,
  ArrowRight,
  Check,
  Box,
  LoaderCircle,
  LockKeyhole,
  Terminal,
  Star,
} from "lucide-react";
import { ApiError, login } from "./api";
import type { App } from "./types";
export { Button } from "./components/arc/button/button";
export { Input } from "./components/arc/input/input";
export { Textarea } from "./components/arc/textarea/textarea";
export { Badge } from "./components/arc/badge/badge";
export { EmptyState } from "./components/arc/empty-state/empty-state";
export { Switch } from "./components/arc/switch/switch";
export { CopyButton } from "./components/arc/copy-button/copy-button";
export { SearchField } from "./components/arc/search-field/search-field";
export { default as SegmentedControl } from "./components/arc/segmented-control/segmented-control";
import { Button } from "./components/arc/button/button";
import { Badge } from "./components/arc/badge/badge";
import { CopyButton } from "./components/arc/copy-button/copy-button";
import { EmptyState } from "./components/arc/empty-state/empty-state";
import { Dialog, DialogContent } from "./components/arc/dialog/dialog";
export function IconLogo({ size = 28 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M7 9h18M7 16h18M7 23h18"
        stroke="currentColor"
        strokeWidth="3.8"
        strokeLinecap="round"
      />
      <path
        d="M12 5v22M20 5v22"
        stroke="currentColor"
        strokeWidth="3.8"
        strokeLinecap="round"
      />
    </svg>
  );
}
export function ErrorNotice({
  error,
  retry,
}: {
  error?: Error;
  retry?: () => void;
}) {
  if (!error) return null;
  return (
    <div className="notice error" role="alert">
      <AlertCircle size={18} />
      <div>
        <strong>{error.message}</strong>
        {error instanceof ApiError && error.hint && <p>{error.hint}</p>}
        {error instanceof ApiError && error.details != null && (
          <details>
            <summary>View exact error details</summary>
            <pre>{JSON.stringify(error.details, null, 2)}</pre>
          </details>
        )}
        {retry && (
          <Button type="button" variant="secondary" size="sm" onClick={retry}>
            Try again
          </Button>
        )}
      </div>
    </div>
  );
}
export function Loading({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="loading" role="status">
      <LoaderCircle size={22} className="spin" />
      <span>{label}</span>
    </div>
  );
}
export function Empty({
  title,
  description,
  action,
  icon,
}: {
  title: string;
  description: string;
  action?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <EmptyState
      title={title}
      description={description}
      action={action}
      icon={icon || <Box />}
      className="empty"
    />
  );
}
export function SignIn({ privateView = false }: { privateView?: boolean }) {
  return (
    <Empty
      title={
        privateView ? "Log in to see private apps" : "Your next app starts here"
      }
      description={
        privateView
          ? "Apps shared with you appear here after you sign in."
          : "Sign in with Silicon Accounts to create apps, manage releases, and build with others."
      }
      icon={<LockKeyhole size={30} />}
      action={
        <Button onClick={login}>
          Sign in with Silicon Accounts <ArrowRight size={16} />
        </Button>
      }
    />
  );
}
export function Command({
  value,
  label = "Copy command",
}: {
  value: string;
  label?: string;
}) {
  return (
    <div className="command">
      <Terminal size={17} />
      <code>{value}</code>
      <CopyButton value={value} label={label} iconOnly variant="plain" />
    </div>
  );
}
export function AppLogo({
  app,
  large = false,
}: {
  app: Pick<App, "name" | "logo" | "logo_alt">;
  large?: boolean;
}) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [app.logo]);
  return (
    <div className={`app-logo ${large ? "large" : ""}`}>
      {app.logo && !failed ? (
        <img
          src={safeUrl(app.logo)}
          alt={app.logo_alt || ""}
          onError={() => setFailed(true)}
        />
      ) : (
        <span>{app.name.slice(0, 1).toUpperCase()}</span>
      )}
    </div>
  );
}
export function AppCard({
  app,
  developer = false,
}: {
  app: App;
  developer?: boolean;
}) {
  return (
    <Link
      to={developer ? `/developer/apps/${app.app_id}` : `/store/${app.app_id}`}
      className="app-card"
    >
      <div className="row between">
        <AppLogo app={app} />
        {developer && !app.published ? (
          <Badge tone="warning">Continue setup</Badge>
        ) : app.visibility === "private" ? (
          <Badge icon={<LockKeyhole size={12} />}>Private</Badge>
        ) : (
          <ArrowRight size={17} className="card-arrow" />
        )}
      </div>
      <div>
        <h3>{app.name}</h3>
        <span className="muted small">{app.app_id}</span>
      </div>
      <p>{app.description || "Add a description to introduce your app."}</p>
      <div className="row between card-meta">
        <span className="row">
          <Star size={14} />
          {app.rating == null ? "No ratings" : app.rating.toFixed(1)}
        </span>
        <span>{app.installs.toLocaleString()} installs</span>
      </div>
    </Link>
  );
}
export function Modal({
  open,
  onClose,
  title,
  description,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: ReactNode;
}) {
  const opener = useRef<HTMLElement | null>(null);
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (!value) onClose();
      }}
    >
      <DialogContent
        title={title}
        description={description}
        className="app-modal"
        onOpenAutoFocus={() => {
          opener.current = document.activeElement as HTMLElement;
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          opener.current?.focus();
        }}
      >
        {children}
      </DialogContent>
    </Dialog>
  );
}
export function SecretModal({
  secret,
  title = "Save your app secret",
  onClose,
}: {
  secret: string | null;
  title?: string;
  onClose: () => void;
}) {
  return (
    <Modal
      open={!!secret}
      onClose={onClose}
      title={title}
      description="This secret is shown only once. Save it somewhere secure before closing this window."
    >
      <div className="stack">
        <div className="secret-value">
          <code>{secret}</code>
        </div>
        <CopyButton value={secret || ""} label="Copy secret" />
        <p className="muted">
          If you lose it, generate a new secret. The previous secret stops
          working immediately.
        </p>
        <Button onClick={onClose}>
          <Check size={16} /> I saved my secret
        </Button>
      </div>
    </Modal>
  );
}
export function PageTitle({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="page-title">
      <div>
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {action}
    </div>
  );
}
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="section">
      <div className="section-heading">
        <h2>{title}</h2>
        {description && <p>{description}</p>}
      </div>
      {children}
    </section>
  );
}
export function SaveStatus({
  status,
  error,
  retry,
}: {
  status: string;
  error?: Error;
  retry?: () => void;
}) {
  return (
    <>
      <div
        className={`save-status ${status === "error" ? "danger" : ""}`}
        role="status"
      >
        {status === "saving" ? (
          <LoaderCircle size={14} className="spin" />
        ) : status === "saved" ? (
          <Check size={14} />
        ) : null}
        {status === "saving"
          ? "Saving changes…"
          : status === "saved"
            ? "All changes saved"
            : status === "error"
              ? "Changes could not be saved"
              : "Unsaved changes"}
      </div>
      <ErrorNotice error={error} retry={retry} />
    </>
  );
}
export function safeUrl(value?: string) {
  if (!value) return undefined;
  try {
    const url = new URL(value, window.location.origin);
    return ["https:", "http:"].includes(url.protocol) ? url.href : undefined;
  } catch {
    return undefined;
  }
}
export function date(value: string) {
  return new Date(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
