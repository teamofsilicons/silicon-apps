/**
 * /playground/error: throws while rendering, to see the error page (app/error.tsx) in the shell. `next dev` only, as
 * the rest of the playground.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";

export const metadata: Metadata = { title: "Playground: Error", robots: { index: false, follow: false } };

export default function PlaygroundError() {
  if (process.env.NODE_ENV === "production") notFound();
  throw new Error("A failure on purpose, to show the error page.");
}
