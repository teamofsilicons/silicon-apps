/**
 * Art the Team likes (Anil, 2026-11-19: "a way to present artworks the team likes"), shown on the 404 for now
 * (app/not-found.tsx, components/parts/art-wall.tsx). Each piece is hung as it is, never cropped, with a wall label
 * crediting the artist and linking to where the work lives. Add a piece by putting its picture in public/art/ (the
 * artist's own file, at the size their site serves) and a line here; keep the credit exactly as the artist gives it.
 */
export type Artwork = {
  title: string;
  artist: string;
  /** Where and when, as the artist writes it ("Japan, 2025"). */
  made: string;
  /** What it is, as the artist describes it. */
  medium?: string;
  /** The work's own page on the artist's site. */
  href: string;
  /** The artist's site, for "More by". */
  artistHref: string;
  picture: { src: string; width: number; height: number; alt: string };
};

export const ARTWORKS: Artwork[] = [
  {
    title: "Untitled I",
    artist: "Austin Augie",
    made: "Japan, 2025",
    medium: "Photograph, printed on 242 gsm Fujifilm Lustre",
    href: "https://www.austinaugie.nyc/product-page/untitled-i-1126",
    artistHref: "https://www.austinaugie.nyc/",
    picture: {
      src: "/art/austin-augie-untitled-i.jpg",
      width: 1876,
      height: 1244,
      alt: "A black-and-white photograph: two people seen from behind through frosted glass, softened to shapes, a third figure faint between them.",
    },
  },
];
