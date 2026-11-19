/**
 * Report a problem (Anil, 2026-11-19: "report a problem in the sidebar bottom, along with developers and docs", as a
 * modal): the sidebar's foot has a Report a problem button that opens this dialog, a native popover, so it opens and
 * closes without script (Escape or a click outside closes it too): what happened, a screenshot if they'd like one
 * (report-screenshot.tsx: take one of this page, or attach an image), an optional pull request, Send Report. The form posts to report-action.ts with the page it was sent from, which comes back with ?report=; the shell
 * shows `ReportNotice` at the top of the page then. Server-rendered.
 */
import { X } from "lucide-react";
import { Alert } from "@teamofsilicons/silicon-ui/alert";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { Input } from "@teamofsilicons/silicon-ui/input";
import { Textarea } from "@teamofsilicons/silicon-ui/textarea";
import { sendReport } from "./report-action";
import { ReportScreenshot } from "./report-screenshot";
import styles from "./report.module.css";

export const REPORT_ID = "report-problem";

const OUTCOMES: Record<string, { tone: "success" | "danger"; title: string; text: string }> = {
  sent: { tone: "success", title: "Your report is sent.", text: "Thank you. It is on its way to the Team." },
  empty: { tone: "danger", title: "Your report wasn’t sent", text: "Tell us what happened before sending the report." },
  long: { tone: "danger", title: "Your report wasn’t sent", text: "Keep the report under 10,000 characters." },
  image: { tone: "danger", title: "Your report wasn’t sent", text: "Attach the screenshot as a PNG, JPEG or WebP image of 5 MB or less." },
  pr: { tone: "danger", title: "Your report wasn’t sent", text: "The pull request link must be an https:// address." },
  unavailable: { tone: "danger", title: "Your report wasn’t sent", text: "Reports are not set up on this server right now. Run silicon-apps report \"what happened\" instead." },
  failed: { tone: "danger", title: "Your report wasn’t sent", text: "We could not send the report. Try again in a moment, or run silicon-apps report \"what happened\"." },
};

/** The page's address without ?report=, so a report sent from a page that just showed a notice doesn't carry it. */
function withoutReport(path: string): string {
  const url = new URL(path, "http://store.local");
  url.searchParams.delete("report");
  return `${url.pathname}${url.search}`;
}

export function ReportDialog({ path }: { path: string }) {
  return (
    <div id={REPORT_ID} popover="auto" className={styles.dialog} data-sq="surface" role="dialog" aria-labelledby={`${REPORT_ID}-title`}>
      <div className={styles.head}>
        <h2 id={`${REPORT_ID}-title`} className={styles.title}>Report a problem</h2>
        <Button variant="ghost" size="sm" popoverTarget={REPORT_ID} popoverTargetAction="hide" aria-label="Close">
          <X size={16} strokeWidth={2} aria-hidden="true" />
        </Button>
      </div>
      <p className={styles.lede}>Tell us what happened and what you expected. If you’ve already fixed it, add the pull request; we’d be grateful.</p>
      <form action={sendReport.bind(null, withoutReport(path), crypto.randomUUID())} className={styles.form}>
        <Textarea label="What happened" name="message" rows={5} required minLength={3} maxLength={10000} />
        <ReportScreenshot dialogId={REPORT_ID} />
        <Input label="Pull request (optional)" name="pr" type="url" placeholder="https://github.com/teamofsilicons/silicon-apps/pull/1" pattern="https://.*" />
        <div className={styles.actions}>
          <Button type="button" variant="secondary" size="sm" popoverTarget={REPORT_ID} popoverTargetAction="hide">Cancel</Button>
          <Button type="submit" size="sm">Send Report</Button>
        </div>
      </form>
    </div>
  );
}

/** After a report, at the top of the page: sent, or why not (?report=). */
export function ReportNotice({ path }: { path: string }) {
  const outcome = OUTCOMES[new URL(path, "http://store.local").searchParams.get("report") ?? ""];
  if (!outcome) return null;
  return (
    <div className={styles.notice}>
      <Alert tone={outcome.tone} title={outcome.title}>{outcome.text}</Alert>
    </div>
  );
}
