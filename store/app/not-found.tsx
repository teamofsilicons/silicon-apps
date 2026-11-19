/** The 404: a gallery wall (components/parts/art-wall.tsx) with a piece from lib/art.ts. Answers 404. */
import type { Metadata } from "next";
import { ArtWall } from "@/components/parts/art-wall";
import { ARTWORKS } from "@/lib/art";

export const metadata: Metadata = { title: "Page Not Found", robots: { index: false } };

export default function NotFound() {
  return <ArtWall art={ARTWORKS[0]} title="This page stepped out of frame." />;
}
