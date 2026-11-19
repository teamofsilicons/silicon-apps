/**
 * The 404 page, in the shell (an unknown address, an app, author or category that doesn't exist or isn't shared with
 * you): lost in the clouds (a sky of dithered pixel clouds, from Cosmos, uploaded by IKIGAIAIR, as the playground picture banner; its rights
 * need checking before shipping, as every Cosmos picture here) with 404 over them, what happened, and the ways on.
 * Answers 404.
 */
import type { Metadata } from "next";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { StatusPage } from "@/components/parts/status-page";

export const metadata: Metadata = { title: "Page Not Found", robots: { index: false } };

export default function NotFound() {
  return (
    <StatusPage
      picture={{ src: "/home/hero-1.webp", width: 1600, height: 905, ink: "#2c5bd4" }}
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
