/**
 * Any address the store cannot show: a real 404 with a way back, server-rendered in full. proxy.ts sends apps and
 * authors the visitor cannot see here too (components/store/not-found-view.tsx tells them apart).
 */
import type { Metadata } from "next";
import { NotFoundView } from "@/components/store/not-found-view";

export const metadata: Metadata = { title: { absolute: "Not found · Silicon Apps" }, robots: { index: false, follow: true } };

export default function NotFound() {
  return <NotFoundView />;
}
