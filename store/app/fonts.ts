/**
 * The type lab's faces (development tools, components/dev/type-lab.tsx), the same list as the account site's lab
 * (silicon-accounts web, app/fonts.ts and lib/type-lab.ts, copied 2026-11-14). The store itself sets everything in the
 * system font (styles/tokens.css); these only put their variables on <html> so the lab can try them. None is preloaded,
 * so a browser fetches a face only when the lab turns it on. All are self-hosted (font-src 'self'): Hanken Grotesk,
 * Geist, Inter and Work Sans through next/font/google (downloaded at build), TeX Gyre Heros (GUST Font License),
 * Liberation Sans and Cooper Hewitt (SIL OFL 1.1) from assets/fonts with their licences, and BDO Grotesk from the
 * Open Graph images' copy in assets/og.
 */
import localFont from "next/font/local";
import { Geist, Hanken_Grotesk, Inter, Work_Sans } from "next/font/google";

const hankenGrotesk = Hanken_Grotesk({ subsets: ["latin", "latin-ext"], variable: "--font-hanken-grotesk", display: "swap", preload: false });
const geist = Geist({ subsets: ["latin", "latin-ext"], variable: "--font-geist", display: "swap", preload: false });
const inter = Inter({ subsets: ["latin", "latin-ext"], variable: "--font-inter", display: "swap", preload: false });
const workSans = Work_Sans({ subsets: ["latin", "latin-ext"], variable: "--font-work-sans", display: "swap", preload: false });

const texGyreHeros = localFont({
  src: [
    { path: "../assets/fonts/tex-gyre-heros/texgyreheros-regular.otf", weight: "400", style: "normal" },
    { path: "../assets/fonts/tex-gyre-heros/texgyreheros-bold.otf", weight: "700", style: "normal" },
  ],
  variable: "--font-tex-gyre-heros",
  display: "swap",
  preload: false,
});

const liberationSans = localFont({
  src: [
    { path: "../assets/fonts/liberation-sans/LiberationSans-Regular.ttf", weight: "400", style: "normal" },
    { path: "../assets/fonts/liberation-sans/LiberationSans-Bold.ttf", weight: "700", style: "normal" },
  ],
  variable: "--font-liberation-sans",
  display: "swap",
  preload: false,
});

const cooperHewitt = localFont({
  src: [
    { path: "../assets/fonts/cooper-hewitt/CooperHewitt-Book.otf", weight: "400", style: "normal" },
    { path: "../assets/fonts/cooper-hewitt/CooperHewitt-Medium.otf", weight: "500", style: "normal" },
    { path: "../assets/fonts/cooper-hewitt/CooperHewitt-Semibold.otf", weight: "600", style: "normal" },
    { path: "../assets/fonts/cooper-hewitt/CooperHewitt-Bold.otf", weight: "700", style: "normal" },
  ],
  variable: "--font-cooper-hewitt",
  display: "swap",
  preload: false,
});

const bdoGrotesk = localFont({
  src: [
    { path: "../assets/og/BDOGrotesk-Regular.ttf", weight: "400", style: "normal" },
    { path: "../assets/og/BDOGrotesk-Medium.ttf", weight: "500", style: "normal" },
    { path: "../assets/og/BDOGrotesk-DemiBold.ttf", weight: "600", style: "normal" },
  ],
  variable: "--font-bdo-grotesk",
  display: "swap",
  preload: false,
});

/** The class names that put the lab's face variables on <html> (development only, app/layout.tsx). */
export const labFontVariables = [hankenGrotesk.variable, geist.variable, inter.variable, workSans.variable, texGyreHeros.variable, liberationSans.variable, cooperHewitt.variable, bdoGrotesk.variable].join(" ");
