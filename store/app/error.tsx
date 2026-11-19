"use client";
/**
 * What a page shows when it fails to render, in the shell: what happened, Try Again (renders the page again, fetching
 * it afresh), Go Home, Report a problem, and the error's reference (its digest, which the server's log carries) to
 * quote in a report. The error's message is never shown: in production it says nothing useful and may say too much.
 */
import { startTransition } from "react";
import { useRouter } from "next/navigation";
import { TriangleAlert } from "lucide-react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { StatusPage } from "@/components/parts/status-page";

export default function ErrorPage({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  const router = useRouter();
  const retry = () => startTransition(() => { router.refresh(); reset(); });
  return (
    <StatusPage
      icon={<TriangleAlert size={24} strokeWidth={1.75} />}
      code="Error"
      title="This page couldn’t load"
      reference={error.digest}
      actions={<>
        <Button type="button" onClick={retry}>Try Again</Button>
        <ButtonLink href="/" variant="secondary">Go Home</ButtonLink>
      </>}
    >
      Something went wrong on our side, not yours. Try again in a moment; if it keeps happening, tell us.
    </StatusPage>
  );
}
