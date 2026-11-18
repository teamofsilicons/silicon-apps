/**
 * Stand-ins the playgrounds show while the catalog has none (2026-11-18): screenshots and reviews. Made up, never
 * served on a real page. Used by /playground/app-header and /playground/app-page.
 */
import type { Media, Review, Reviews as ReviewList } from "@/lib/types";

/** Stand-ins for screenshots while no app in the catalog has any (2026-11-18): the store's own pictures, three in desktop
 * frames and two in mobile ones, so the strip's ratios and a mix of the two can be judged. Not screenshots. */
export const SAMPLE_SHOTS: Media[] = [
  { url: "/home/hero-2.webp", kind: "image", form: "desktop", alt: "Sample desktop screenshot" },
  { url: "/home/silicon-apps.webp", kind: "image", form: "mobile", alt: "Sample mobile screenshot" },
  { url: "/home/hero-1.webp", kind: "image", form: "desktop", alt: "Sample desktop screenshot" },
  { url: "/home/silicon-accounts.webp", kind: "image", form: "mobile", alt: "Sample mobile screenshot" },
  { url: "/banner/bg-110.webp", kind: "image", form: "desktop", alt: "Sample desktop screenshot" },
];

/** Stand-ins for reviews while no app in the catalog has any (2026-11-18): eight, so the newest six show and the rest
 * fold, with a short one, a long one, one without words and every star count. Made up; not anyone's words. */
export const SAMPLE_REVIEWS: Review[] = [
  { uuid: "s1", id: "c:sample-ana", rating: 5, text: "Does exactly what it says. Installed in one command and it has stayed out of my way since.", updated_at: "2026-11-17T10:00:00Z" },
  { uuid: "s2", id: "si:sample-scout", rating: 4, text: "Quick to set up from the CLI. I would like a way to export everything as JSON, but the rest is solid.", updated_at: "2026-11-15T09:00:00Z" },
  { uuid: "s3", id: "c:sample-ravi", rating: 5, text: "", updated_at: "2026-11-12T12:00:00Z" },
  { uuid: "s4", id: "c:sample-lena", rating: 3, text: "Works well on macOS. On Linux the first run asked me to sign in twice before it remembered me, which was confusing. Once past that it has been reliable, and updates arrive without me thinking about them. The docs could say more about running it on a server without a browser nearby.", updated_at: "2026-11-08T08:00:00Z" },
  { uuid: "s5", id: "si:sample-relay", rating: 5, text: "Clean API, sensible errors. My agent picked it up without extra instructions.", updated_at: "2026-11-02T16:00:00Z" },
  { uuid: "s6", id: "c:sample-jo", rating: 2, text: "Missing the one feature I needed. Will try again later.", updated_at: "2026-10-28T11:00:00Z" },
  { uuid: "s7", id: "c:sample-omar", rating: 4, text: "Good. Small, fast and does one thing.", updated_at: "2026-10-20T14:00:00Z" },
  { uuid: "s8", id: "si:sample-atlas", rating: 1, text: "Failed to install on an older Intel Mac.", updated_at: "2026-10-11T07:00:00Z" },
];
export const SAMPLE_LIST: ReviewList = { items: SAMPLE_REVIEWS, count: SAMPLE_REVIEWS.length, rating: SAMPLE_REVIEWS.reduce((sum, item) => sum + item.rating, 0) / SAMPLE_REVIEWS.length };
