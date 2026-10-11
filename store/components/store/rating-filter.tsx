"use client";
/**
 * The rating filter as five stars: point at a star (or its left half) and the stars fill to show "4.5 stars or more",
 * click to keep it, "Any rating" to let it go. The keyboard moves by half a star with the arrow keys and keeps it with
 * Enter or Space. It opens from a quiet trigger in the filter row, like the other filters (Arc's popover surface).
 */
import { useState, type KeyboardEvent } from "react";
import * as Popover from "@radix-ui/react-popover";
import { ChevronDown } from "lucide-react";
import { MIN_RATINGS, ratingLabel } from "@/lib/filters";
import styles from "./search-toolbar.module.css";

const STAR = "M12 2.6l2.9 5.9 6.5.9-4.7 4.6 1.1 6.5L12 17.4l-5.8 3.1 1.1-6.5L2.6 9.4l6.5-.9z";

/** One star, filled from the left by `fill` (0, .5 or 1). */
function StarShape({ fill, id }: { fill: number; id: string }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" className={styles.starShape}>
      <defs>
        <clipPath id={id}><rect x="0" y="0" width={24 * fill} height="24" /></clipPath>
      </defs>
      <path d={STAR} className={styles.starEmpty} />
      <path d={STAR} className={styles.starFull} clipPath={`url(#${id})`} />
    </svg>
  );
}

export function RatingFilter({ value, onChange }: { value: number; onChange: (value: number) => void }) {
  const [open, setOpen] = useState(false);
  const [hover, setHover] = useState<number | null>(null);
  const [focus, setFocus] = useState(value || 4);
  // The keyboard's position shows only once the keyboard has moved; until then the stars show the current filter.
  const [keyed, setKeyed] = useState(false);
  const shown = hover ?? (keyed ? focus : value);
  const choose = (next: number) => {
    onChange(next);
    setOpen(false);
    setHover(null);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    setKeyed(true);
    const index = MIN_RATINGS.indexOf(focus as (typeof MIN_RATINGS)[number]);
    if (event.key === "ArrowRight" || event.key === "ArrowUp") { event.preventDefault(); setFocus(MIN_RATINGS[Math.min(MIN_RATINGS.length - 1, index + 1)]); }
    else if (event.key === "ArrowLeft" || event.key === "ArrowDown") { event.preventDefault(); setFocus(MIN_RATINGS[Math.max(0, index - 1)]); }
    else if (event.key === "Home") { event.preventDefault(); setFocus(MIN_RATINGS[0]); }
    else if (event.key === "End") { event.preventDefault(); setFocus(MIN_RATINGS[MIN_RATINGS.length - 1]); }
    else if (event.key === "Enter" || event.key === " ") { event.preventDefault(); choose(focus); }
  };
  return (
    <Popover.Root open={open} onOpenChange={next => { setOpen(next); if (next) setFocus(value || 4); setHover(null); setKeyed(false); }}>
      <span className={styles.filter} data-active={value ? "" : undefined}>
        <Popover.Trigger className={styles.trigger} type="button">
          {value ? (
            <>
              <span className={styles.triggerStar} aria-hidden="true">★</span>
              <span>{value >= 5 ? "5.0" : `${value.toFixed(1)}+`}</span>
              <span className="sr-only">{ratingLabel(value)}</span>
            </>
          ) : "Rating"}
          <ChevronDown size={15} strokeWidth={1.8} aria-hidden="true" className={styles.chevron} />
        </Popover.Trigger>
      </span>
      <Popover.Portal>
        <Popover.Content className={styles.popover} align="start" sideOffset={6} collisionPadding={12}>
          <p className={styles.ratingCaption} aria-live="polite">{shown ? ratingLabel(shown) : "Any rating"}</p>
          <div
            className={styles.stars}
            role="slider"
            tabIndex={0}
            aria-label="Minimum rating"
            aria-valuemin={1}
            aria-valuemax={5}
            aria-valuenow={focus}
            aria-valuetext={ratingLabel(focus)}
            onKeyDown={onKeyDown}
            onPointerLeave={() => setHover(null)}
          >
            {[1, 2, 3, 4, 5].map(star => {
              const fill = Math.max(0, Math.min(1, shown - (star - 1)));
              return (
                <span key={star} className={styles.star} data-lit={fill > 0 ? "" : undefined}>
                  <StarShape fill={fill} id={`rating-star-${star}`} />
                  {/* Two halves to point at: the left half is half a star less. */}
                  <button type="button" tabIndex={-1} aria-hidden="true" className={styles.half} data-side="left" onPointerEnter={() => setHover(star - 0.5 < 1 ? 1 : star - 0.5)} onClick={() => choose(star - 0.5 < 1 ? 1 : star - 0.5)} />
                  <button type="button" tabIndex={-1} aria-hidden="true" className={styles.half} data-side="right" onPointerEnter={() => setHover(star)} onClick={() => choose(star)} />
                </span>
              );
            })}
          </div>
          <button type="button" className={styles.anyRating} onClick={() => choose(0)} data-on={value ? undefined : ""}>Any rating</button>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
