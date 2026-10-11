/**
 * The Silicon Apps mark: seven hexagons, one in the middle and six around it. The header draws the bare glyph in ink
 * beside the wordmark, as Silicon Writings does; BrandMark is the app icon (cream hexagons on #181818), the same
 * drawing as public/icon.svg and the PNG icons. Plain SVG, server-rendered.
 */
const HEXAGONS = [
  "M77 100L88.5 80.0814H111.5L123 100L111.5 119.919H88.5L77 100Z",
  "M115 122L126.5 102.081H149.5L161 122L149.5 141.919H126.5L115 122Z",
  "M77 56L88.5 36.0814L111.5 36.0814L123 56L111.5 75.9186H88.5L77 56Z",
  "M115 78L126.5 58.0814L149.5 58.0814L161 78L149.5 97.9186H126.5L115 78Z",
  "M39 78L50.5 58.0814L73.5 58.0814L85 78L73.5 97.9186H50.5L39 78Z",
  "M39 122L50.5 102.081H73.5L85 122L73.5 141.919H50.5L39 122Z",
  "M77 144L88.5 124.081H111.5L123 144L111.5 163.919H88.5L77 144Z",
];

/** The seven hexagons alone, in currentColor, cropped to their own bounds. */
export function BrandGlyph() {
  return (
    <svg viewBox="36 33 128 134" fill="currentColor" aria-hidden="true" focusable="false">
      {HEXAGONS.map(d => <path key={d} d={d} />)}
    </svg>
  );
}

/** The app icon: the hexagons in cream on a #181818 square. */
export function BrandMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 200 200" aria-hidden="true" focusable="false">
      <rect width="200" height="200" fill="#181818" />
      {HEXAGONS.map(d => <path key={d} d={d} fill="#EDE8E0" />)}
    </svg>
  );
}

export { HEXAGONS };
