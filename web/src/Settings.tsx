import { useEffect, useState } from "react";
import { Bug, Check, ExternalLink, LogOut } from "lucide-react";
import { api, login, useMutation } from "./api";
import { useSession } from "./context";
import {
  Button,
  ErrorNotice,
  Input,
  PageTitle,
  Section,
  Switch,
  Textarea,
} from "./ui";
export function telemetryEnabled() {
  return localStorage.getItem("apps.telemetry") !== "false";
}
export async function telemetry(
  event: string,
  context: Record<string, unknown> = {},
) {
  if (!telemetryEnabled()) return;
  try {
    await api("/telemetry", {
      method: "POST",
      body: {
        source: "apps-web",
        step: event,
        progress: "complete",
        event,
        ...context,
      },
    });
  } catch {
    /* Telemetry is best effort and never interrupts app work. */
  }
}
export function Settings() {
  const { account, refresh } = useSession();
  const [telemetryOn, setTelemetryOn] = useState(true);
  const [theme, setTheme] = useState("light");
  const [message, setMessage] = useState("");
  const [pr, setPr] = useState("");
  const [sent, setSent] = useState(false);
  const mutation = useMutation();
  const logout = useMutation(refresh);
  useEffect(() => {
    setTelemetryOn(telemetryEnabled());
    setTheme(localStorage.getItem("apps.theme") || "light");
  }, []);
  return (
    <>
      <PageTitle
        title="Settings"
        description="A few things to make Silicon Apps feel like yours."
      />
      <div className="settings-page">
        <Section
          title="Silicon Accounts"
          description={
            account
              ? `Signed in as ${account.id}`
              : "Sign in to discover private apps, write reviews, and publish your work."
          }
        >
          {account ? (
            <div className="row between">
              <div className="row">
                <div className="avatar">
                  {(account.display_name || account.id)
                    .slice(0, 1)
                    .toUpperCase()}
                </div>
                <div>
                  <strong>{account.display_name || account.id}</strong>
                  <p className="muted small">{account.id}</p>
                </div>
              </div>
              <Button
                variant="secondary"
                loading={logout.pending}
                onClick={() =>
                  void logout.run(() =>
                    api("/auth/logout", { method: "POST", body: {} }),
                  )
                }
              >
                <LogOut size={16} /> Sign out
              </Button>
            </div>
          ) : (
            <Button onClick={login}>Sign in with Silicon Accounts</Button>
          )}
          <ErrorNotice error={logout.error} />
        </Section>
        <Section title="Appearance">
          <label className="field-label">
            Theme
            <select
              className="native-input"
              value={theme}
              onChange={(e) => {
                const value = e.target.value;
                setTheme(value);
                localStorage.setItem("apps.theme", value);
                document.documentElement.dataset.theme = value;
              }}
            >
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
        </Section>
        <Section
          title="Telemetry"
          description="Help improve Silicon Apps by sending usage events to Space Station. Events describe the source, step, and progress; app secrets and form contents are never included."
        >
          <div className="row between">
            <div>
              <strong>Share usage telemetry</strong>
              <p className="small muted">
                Enabled by default. This preference applies to this browser.
              </p>
            </div>
            <Switch
              aria-label="Share usage telemetry"
              checked={telemetryOn}
              onCheckedChange={(value) => {
                setTelemetryOn(value);
                localStorage.setItem("apps.telemetry", String(value));
              }}
            />
          </div>
        </Section>
        <Section
          title="Report a problem"
          description="Tell us what happened and what you expected. You can include a pull request if you already have a fix."
        >
          <form
            className="form-stack"
            onSubmit={async (e) => {
              e.preventDefault();
              const result = await mutation.run(() =>
                api("/reports", {
                  method: "POST",
                  body: { message, pr: pr || undefined },
                }),
              );
              if (result) {
                setSent(true);
                setMessage("");
                setPr("");
              }
            }}
          >
            <Textarea
              label="Report"
              value={message}
              onChange={(e) => {
                setMessage(e.target.value);
                setSent(false);
              }}
              required
              rows={5}
            />
            <Input
              label="Pull request URL (optional)"
              type="url"
              value={pr}
              onChange={(e) => setPr(e.target.value)}
              placeholder="https://github.com/…"
            />
            <ErrorNotice error={mutation.error} />
            <Button
              type="submit"
              disabled={!message.trim()}
              loading={mutation.pending}
            >
              <Bug size={16} /> Send report
            </Button>
            {sent && (
              <p className="success row" role="status">
                <Check size={16} /> Your report has been queued for the team.
              </p>
            )}
          </form>
        </Section>
      </div>
    </>
  );
}
