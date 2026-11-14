"use client";
/**
 * The banner carousel's controls (banner-carousel.tsx), drawn on the banner so they take no room of their own (Anil,
 * 2026-11-14: sleek, nothing weighing on the page): small dots in its lower right, each a plain link to its banner (so
 * they work without script), and, with script, round Previous and Next at its sides that fade in on hover or focus,
 * the banner in view's dot stretched into a short pill, and a dot click that scrolls the strip itself rather than
 * jumping the page. Smooth unless the visitor prefers reduced
 * motion.
 */
import { useEffect, useState, type MouseEvent } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@teamofsilicons/silicon-ui/button";
import styles from "./banner-carousel.module.css";

export function CarouselControls({ trackId, slides }: { trackId: string; slides: { id: string; title: string }[] }) {
  const [current, setCurrent] = useState(0);

  useEffect(() => {
    const track = document.getElementById(trackId);
    if (!track) return;
    const update = () => setCurrent(Math.round(track.scrollLeft / Math.max(1, track.clientWidth)));
    update();
    track.addEventListener("scroll", update, { passive: true });
    return () => track.removeEventListener("scroll", update);
  }, [trackId]);

  const go = (index: number) => {
    const track = document.getElementById(trackId);
    if (!track) return;
    const next = (index + slides.length) % slides.length;
    const smooth = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    track.scrollTo({ left: next * track.clientWidth, behavior: smooth ? "smooth" : "auto" });
  };
  const onDot = (event: MouseEvent, index: number) => {
    event.preventDefault();
    go(index);
  };

  return (
    <>
      <span className={styles.prev} data-js-only="">
        <Button variant="ghost" size="sm" className={styles.arrow} aria-controls={trackId} aria-label="Previous banner" onClick={() => go(current - 1)}>
          <ChevronLeft size={16} strokeWidth={2} aria-hidden="true" />
        </Button>
      </span>
      <span className={styles.next} data-js-only="">
        <Button variant="ghost" size="sm" className={styles.arrow} aria-controls={trackId} aria-label="Next banner" onClick={() => go(current + 1)}>
          <ChevronRight size={16} strokeWidth={2} aria-hidden="true" />
        </Button>
      </span>
      <nav className={styles.dots} aria-label="Banners">
        {slides.map((slide, index) => (
          <a key={slide.id} href={`#${slide.id}`} className={styles.dot} aria-label={slide.title} aria-current={index === current ? "true" : undefined} onClick={event => onDot(event, index)} />
        ))}
      </nav>
    </>
  );
}
