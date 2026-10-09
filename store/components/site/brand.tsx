/**
 * The Silicon Apps mark: a brand-blue squircle with four app tiles, as the developer site has braces and the account
 * site a person (the same squircle, the same white glyph). Plain SVG, server-rendered; public/icon.svg and the other
 * icons are the same glyph for the favicon and the home screen.
 */
export function BrandGlyph() {
  return (
    <svg viewBox="8 8 48 48" fill="currentColor" aria-hidden="true" focusable="false">
      <rect x="15" y="15" width="15" height="15" rx="4.5" />
      <rect x="34" y="15" width="15" height="15" rx="4.5" />
      <rect x="15" y="34" width="15" height="15" rx="4.5" />
      <rect x="34" y="34" width="15" height="15" rx="4.5" />
    </svg>
  );
}

export function BrandMark({ className }: { className?: string }) {
  return (
    <span data-sq="clip" className={className} aria-hidden="true">
      <BrandGlyph />
    </span>
  );
}
