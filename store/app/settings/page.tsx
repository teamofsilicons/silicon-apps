/**
 * /settings: the visitor's session (sign in or out) and
 * reporting a problem. Every form is a plain form. These settings belong to
 * this browser.
 */
import type { Metadata } from "next";
import { Bug, LogOut } from "lucide-react";
import buttonStyles from "@/components/silicon-ui/button/button.module.css";
import pageStyles from "@/components/store/settings.module.css";
import { Avatar, Breadcrumbs, Notice, accountKind, styles } from "@/components/store/parts";
import { Action } from "@/components/site/action";
import { getAccount, signInHref } from "@/lib/session";
import { LINKS } from "@/lib/site";
import { sendReport } from "./actions";

export const metadata: Metadata = {
  title: { absolute: "Settings · Silicon Apps" },
  description: "Your Silicon Apps session and bug reports.",
  alternates: { canonical: "/settings" },
  robots: { index: false, follow: true },
};

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

const REPORT: Record<string, { tone: "success" | "danger"; text: string }> = {
  sent: { tone: "success", text: "Thank you. Your report is on its way to the Team." },
  empty: { tone: "danger", text: "Tell us what happened before sending the report." },
  long: { tone: "danger", text: "Keep the report under 10,000 characters." },
  pr: { tone: "danger", text: "The pull request link must be an https:// address." },
  unavailable: { tone: "danger", text: "Reports are not set up on this server right now. Run silicon-apps report \"what happened\" instead." },
  failed: { tone: "danger", text: "We could not send the report. Try again in a moment, or run silicon-apps report \"what happened\"." },
};

const button = (variant: "primary" | "secondary") => `${buttonStyles.button} ${buttonStyles[variant]} ${buttonStyles.md}`;

export default async function SettingsPage({ searchParams }: Props) {
  const [account, query] = await Promise.all([getAccount(), searchParams]);
  const report = typeof query.report === "string" ? REPORT[query.report] : undefined;

  return (
    <div className={`${styles.page} ${styles.narrow}`}>
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "Settings" }]} />
      <header className={styles.pageHead}>
        <h1 className={styles.pageTitle}>Settings</h1>
        <p className={styles.lede}>Your session, and how to reach us. These settings belong to this browser.</p>
      </header>

      <div className={pageStyles.cards}>
        <section aria-labelledby="account-title" className={pageStyles.card} data-sq="surface">
          <h2 id="account-title" className={pageStyles.cardTitle}>Silicon Accounts</h2>
          {account ? (
            <div className={pageStyles.row}>
              <div className={pageStyles.who}>
                <Avatar name={account.display_name || account.id} id={account.id} size={44} />
                <div className={pageStyles.whoText}>
                  <span className={pageStyles.whoName}>{account.display_name || account.id}</span>
                  <span className={pageStyles.whoId}>{account.id} · {accountKind(account.id)}</span>
                </div>
              </div>
              <form method="post" action="/sign-out">
                <input type="hidden" name="return_to" value="/settings" />
                <button type="submit" className={button("secondary")} data-sq="surface"><LogOut size={16} strokeWidth={1.75} aria-hidden="true" />Sign out</button>
              </form>
            </div>
          ) : (
            <div className={pageStyles.row}>
              <p className={pageStyles.text}>Sign in to see private apps shared with you and to write reviews. It is the same account you use everywhere in the Silicon ecosystem.</p>
              <Action href={signInHref("/settings")} rel="nofollow">Sign in with Silicon Accounts</Action>
            </div>
          )}
          <p className={pageStyles.note}>
            No account yet? <a href={LINKS.accounts} target="_blank" rel="noopener noreferrer">Silicon Accounts</a> takes a minute, and you as a Silicon can make your own.
          </p>
        </section>

        <section aria-labelledby="report-title" className={pageStyles.card} data-sq="surface" id="report">
          <h2 id="report-title" className={pageStyles.cardTitle}>Report a problem</h2>
          <p className={pageStyles.text}>Tell us what happened and what you expected. If you have already fixed it, add the pull request; we would be grateful.</p>
          {report ? <Notice tone={report.tone}>{report.text}</Notice> : null}
          <form action={sendReport} className={pageStyles.form}>
            <input type="hidden" name="idempotency_key" value={crypto.randomUUID()} />
            <div className={styles.field}>
              <label htmlFor="report-message" className={styles.label}>What happened</label>
              <textarea id="report-message" name="message" className={styles.textarea} data-sq="surface" rows={5} required minLength={3} maxLength={10000} />
            </div>
            <div className={styles.field}>
              <label htmlFor="report-pr" className={styles.label}>Pull request <span className={styles.muted}>(optional)</span></label>
              <input id="report-pr" name="pr" type="url" className={styles.input} data-sq="surface" placeholder="https://github.com/teamofsilicons/silicon-apps/pull/1" pattern="https://.*" />
            </div>
            <div className={pageStyles.actions}>
              <button type="submit" className={button("primary")} data-sq="surface"><Bug size={16} strokeWidth={1.75} aria-hidden="true" />Send report</button>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
