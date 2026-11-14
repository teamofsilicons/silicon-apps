/**
 * /playground: designs kept to compare while the home page is designed (Anil, 2026-11-14), under `next dev` only (a
 * 404 in production builds, never indexed).
 * - The home page's earlier banners: a wall of app icons, and two Cosmos pictures cross-fading.
 * - The banner carousel, off the home page for now (2026-11-14), with its three banners in both styles: the words
 *   under the picture, and the words in white on the picture over an overlay.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { BannerCarousel } from "@/components/home/banner-carousel";
import { BANNERS } from "@/components/home/banners";
import { IconHero } from "@/components/home/icon-hero";
import { PictureHero } from "@/components/home/picture-hero";
import { listAll } from "@/lib/catalog";
import type { App } from "@/lib/types";
import { Page } from "@/components/parts/page";
import styles from "./playground.module.css";

export const metadata: Metadata = { title: "Playground", robots: { index: false, follow: false } };

export default async function Playground() {
  if (process.env.NODE_ENV === "production") notFound();
  const apps = await listAll().catch((): App[] => []);
  return (
    <Page>
      <h1 className="sr-only">Playground</h1>
      <section className={styles.item} aria-labelledby="icon-hero">
        <h2 id="icon-hero" className={styles.label}>Home banner: a wall of app icons</h2>
        <IconHero apps={apps} />
      </section>
      <section className={styles.item} aria-labelledby="picture-hero">
        <h2 id="picture-hero" className={styles.label}>Home banner: two pictures cross-fading</h2>
        <PictureHero />
      </section>
      <section className={styles.item} aria-labelledby="below-banner">
        <h2 id="below-banner" className={styles.label}>Banner: words under the picture</h2>
        <BannerCarousel banners={BANNERS} label="About Silicon Apps" variant="below" id="playground-below" />
      </section>
      <section className={styles.item} aria-labelledby="overlay-banner">
        <h2 id="overlay-banner" className={styles.label}>Banner: words on the picture, with an overlay</h2>
        <BannerCarousel banners={BANNERS} label="About Silicon Apps" variant="overlay" id="playground-overlay" />
      </section>
    </Page>
  );
}
