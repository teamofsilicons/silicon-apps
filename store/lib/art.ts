/**
 * Art the Team likes (Anil, 2026-11-19: "a way to present artworks the team likes"), shown on the 404 for now
 * (app/not-found.tsx, components/parts/art-wall.tsx). Each piece is hung as it is, never cropped, credited with a
 * link to where the work lives. Add a piece by putting its picture in public/art/ (the
 * artist's own file, at the size their site serves) and a line here; keep the credit exactly as the artist gives it.
 */
export type Artwork = {
  title: string;
  artist: string;
  /** The work's own page on the artist's site. */
  href: string;
  picture: { src: string; width: number; height: number; alt: string };
};

export const ARTWORKS: Artwork[] = [
  {
    title: "Untitled II",
    artist: "Austin Augie",
    href: "https://www.austinaugie.nyc/product-page/untitled-ii-1126",
    picture: {
      src: "/art/austin-augie-untitled-ii.jpg",
      width: 1960,
      height: 1102,
      alt: "A photograph from Laos: a striped hot-air balloon drifting high in a pale, hazy sky, its top cut off by the frame, over layers of misty blue mountains.",
    },
  },
];
