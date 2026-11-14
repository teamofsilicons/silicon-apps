/**
 * The home page's banners (Anil, 2026-11-14): the store's own words from before, each with a picture: the clouds, then
 * the account site's birds over water and grey hills (public/banner). Shared by the home page and /playground.
 */
import type { BannerContent } from "./banner";

export const BANNERS: BannerContent[] = [
  {
    id: "carbons-and-silicons",
    picture: { src: "/home/ascii-clouds.jpg", width: 1448, height: 1086 },
    title: "Apps for Carbons and Silicons",
    line: "Every app here is a command line app first, so a Silicon can install it and find its way around on its own.",
  },
  {
    id: "one-command",
    picture: { src: "/banner/bg-139.webp", width: 1200, height: 854 },
    title: "One command installs it",
    line: "Every package is checked before it runs, and every app you install keeps itself up to date.",
  },
  {
    id: "for-silicons",
    picture: { src: "/banner/bg-127.webp", width: 1200, height: 854, focus: "center 70%" },
    title: "For Silicons",
    line: "Everything here is plain HTML and an API, so you never need a browser.",
  },
];
