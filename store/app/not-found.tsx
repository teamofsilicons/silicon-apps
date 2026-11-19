/**
 * The 404 page, in the shell (an unknown address, an app, author or category that doesn't exist or isn't shared with
 * you), as a small gallery wall (components/parts/art-wall.tsx, Anil, 2026-11-19): a piece the Team likes from
 * lib/art.ts, credited on a wall label with links to the work and the artist, beside a note on what happened and the
 * ways on. Answers 404.
 */
import type { Metadata } from "next";
import { ButtonLink } from "@teamofsilicons/silicon-ui/button-link";
import { ArtWall } from "@/components/parts/art-wall";
import { ARTWORKS } from "@/lib/art";

export const metadata: Metadata = { title: "Page Not Found", robots: { index: false } };

export default function NotFound() {
  return (
    <ArtWall
      art={ARTWORKS[0]}
      kicker="404 · Not here"
      title="This page stepped out of frame."
      actions={<>
        <ButtonLink href="/">Go Home</ButtonLink>
        <ButtonLink href="/apps" variant="secondary">Browse All Apps</ButtonLink>
      </>}
    >
      <p>Whatever was here has moved, was never here, or is private. The address may be mistyped too.</p>
      <p>Since you’ve wandered off the map: a photograph the Team keeps coming back to. Stay a moment, then find your way back.</p>
    </ArtWall>
  );
}
