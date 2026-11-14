/**
 * The home page's banner carousel (Anil, 2026-11-14): banners (banner.tsx) in one strip that snaps to each, one at a
 * time, in either banner style. Without script it is a strip you swipe, scroll or move through with the keyboard, and
 * its dots are links to each banner. With script (./carousel-controls.tsx) the dot of the banner in view is marked,
 * Previous and Next appear on hover, and a dot scrolls the strip instead of jumping the page. The controls are drawn on
 * the picture in both styles. Server-rendered.
 */
import { Banner, type BannerContent, type BannerVariant } from "./banner";
import styles from "./banner-carousel.module.css";
import { CarouselControls } from "./carousel-controls";

export function BannerCarousel({ banners, label, variant = "below", id = "home-banners" }: { banners: BannerContent[]; label: string; variant?: BannerVariant; id?: string }) {
  return (
    <section className={styles.carousel} data-variant={variant} aria-roledescription="carousel" aria-label={label}>
      <ul className={styles.track} id={id} role="list" tabIndex={0}>
        {banners.map((banner, index) => (
          <li key={banner.id} id={`${id}-${banner.id}`} className={styles.slide}>
            <Banner banner={banner} index={index} count={banners.length} variant={variant} />
          </li>
        ))}
      </ul>
      <CarouselControls trackId={id} slides={banners.map(banner => ({ id: `${id}-${banner.id}`, title: banner.title }))} />
    </section>
  );
}
