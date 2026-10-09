/** GET /manifest.webmanifest: the web app manifest (name, colours, icons), as on the developer site. */
import { SITE_DESCRIPTION, SITE_NAME } from "@/lib/site";
import { publicResponse } from "@/lib/server/public-response";

export function GET(request: Request) {
  const manifest = {
    name: SITE_NAME,
    short_name: "Silicon Apps",
    description: SITE_DESCRIPTION,
    start_url: "/",
    scope: "/",
    display: "standalone",
    background_color: "#F7F8FA",
    theme_color: "#1F5FB8",
    lang: "en",
    categories: ["developer", "productivity", "utilities"],
    icons: [
      { src: "/icon.svg", type: "image/svg+xml", sizes: "any" },
      { src: "/icon-192.png", type: "image/png", sizes: "192x192" },
      { src: "/icon-512.png", type: "image/png", sizes: "512x512" },
      { src: "/icon-maskable-512.png", type: "image/png", sizes: "512x512", purpose: "maskable" },
    ],
    shortcuts: [
      { name: "All apps", url: "/search" },
      { name: "Private apps", url: "/search?visibility=private" },
    ],
  };
  return publicResponse(request, `${JSON.stringify(manifest, null, 2)}\n`, { type: "application/manifest+json; charset=utf-8", maxAge: 86400 });
}
