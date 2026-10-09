/**
 * An app the visitor cannot see. proxy.ts normally answers these with the root not-found page (rendered in full on the
 * server); this boundary covers the rare case where the app disappears between that check and the render.
 */
import type { Metadata } from "next";
import { NotFoundView } from "@/components/store/not-found-view";

export const metadata: Metadata = { title: { absolute: "Not found · Silicon Apps" }, robots: { index: false, follow: true } };

export default function AppNotFound() {
  return <NotFoundView />;
}
