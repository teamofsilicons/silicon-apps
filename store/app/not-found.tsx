/**
 * The 404 page, in the shell (an unknown address, an app, author or category that doesn't exist or isn't shared with
 * you): what happened, and the ways on. Answers 404.
 */
import type { Metadata } from "next";
import { SearchX } from "lucide-react";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { StatusPage } from "@/components/parts/status-page";

export const metadata: Metadata = { title: "Page Not Found", robots: { index: false } };

export default function NotFound() {
  return (
    <StatusPage
      icon={<SearchX size={24} strokeWidth={1.75} />}
      code="404"
      title="This page isn’t here"
      actions={<>
        <ButtonLink href="/">Go Home</ButtonLink>
        <ButtonLink href="/apps" variant="secondary">Browse All Apps</ButtonLink>
      </>}
    >
      The address may be mistyped, or the app was removed or is private. Try searching for it, or browse every app.
    </StatusPage>
  );
}
