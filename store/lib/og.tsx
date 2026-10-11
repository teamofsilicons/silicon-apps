/**
 * An app's Open Graph image (1200 by 630 PNG), drawn with next/og in the look of the store's own social image
 * (public/og.png): BDO Grotesk on warm paper, the app's logo, name and description, and the command that installs it.
 *
 * Only public data is drawn. Logos are used only when they are data: images or this store's own API media read
 * through APPS_API_URL (never an arbitrary remote URL, so this server fetches nothing it was not built for).
 */
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import { HEXAGONS } from "@/components/site/brand";
import { appsApiUrl } from "./config";
import { formatCount, formatRating, monogramOf, operatingSystems, truncate } from "./format";
import type { App } from "./types";

const fonts = (async () => {
  const dir = join(process.cwd(), "assets", "og");
  const [regular, medium, demi] = await Promise.all(["BDOGrotesk-Regular.ttf", "BDOGrotesk-Medium.ttf", "BDOGrotesk-DemiBold.ttf"].map(name => readFile(join(dir, name))));
  const data = (buffer: Buffer) => buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength) as ArrayBuffer;
  return [
    { name: "BDO Grotesk", data: data(regular), weight: 400 as const, style: "normal" as const },
    { name: "BDO Grotesk", data: data(medium), weight: 500 as const, style: "normal" as const },
    { name: "BDO Grotesk", data: data(demi), weight: 600 as const, style: "normal" as const },
  ];
})();

const PAGE = "#FFFDF9";
const INK = "#1E1E1C";
const SECONDARY = "#57544D";
const BORDER = "#E7E3D8";
const BLUE = "#1F5CB1";

/** The Silicon Apps mark (components/site/brand.tsx): cream hexagons on #181818. */
function Mark({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200">
      <rect width="200" height="200" rx="36" fill="#181818" />
      {HEXAGONS.map(d => <path key={d} d={d} fill="#EDE8E0" />)}
    </svg>
  );
}

/** A logo this server may draw: a data: image, or the store's own API media read through APPS_API_URL. */
async function logoSource(logo: string): Promise<string | null> {
  const value = logo.trim();
  if (/^data:image\/(png|jpe?g|svg\+xml);base64,/i.test(value) && value.length < 1_500_000) return value;
  const media = /^(?:https:\/\/apps\.teamofsilicons\.com)?(\/v1\/apps\/[a-z0-9_-]{1,64}\/media\/[a-f0-9]{64})$/.exec(value);
  if (!media) return null;
  try {
    const response = await fetch(`${appsApiUrl()}${media[1]}`, { cache: "no-store", signal: AbortSignal.timeout(4_000) });
    const type = (response.headers.get("content-type") || "").split(";")[0].trim();
    if (!response.ok || !/^image\/(png|jpeg)$/.test(type)) return null;
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.length > 1_000_000) return null;
    return `data:${type};base64,${bytes.toString("base64")}`;
  } catch {
    return null;
  }
}

export async function appImage(app: App): Promise<ImageResponse> {
  const logo = app.logo ? await logoSource(app.logo) : null;
  const stats = [
    app.review_count > 0 && app.rating !== null ? `${formatRating(app.rating)} stars` : null,
    `${formatCount(app.installs)} ${app.installs === 1 ? "install" : "installs"}`,
    operatingSystems(app.targets).join(", ") || null,
  ].filter((value): value is string => Boolean(value));
  return new ImageResponse(
    (
      <div style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column", padding: "64px 76px", backgroundColor: PAGE, color: INK, fontFamily: "BDO Grotesk" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 16, fontSize: 30, fontWeight: 600, letterSpacing: -0.6 }}>
          <Mark size={52} />
          <span>Silicon</span>
          <span style={{ color: SECONDARY, fontWeight: 500, marginLeft: -6 }}>Apps</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 44, marginTop: 64 }}>
          {logo ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={logo} width={176} height={176} alt="" style={{ borderRadius: 44, border: `1.5px solid ${BORDER}` }} />
          ) : (
            <div style={{ width: 176, height: 176, borderRadius: 44, display: "flex", alignItems: "center", justifyContent: "center", fontSize: 92, fontWeight: 500, backgroundColor: "#EDE8E0", color: BLUE }}>{monogramOf(app.name)}</div>
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 800 }}>
            <div style={{ fontSize: 80, fontWeight: 600, letterSpacing: -3, lineHeight: 1 }}>{truncate(app.name, 30)}</div>
            <div style={{ fontSize: 30, color: SECONDARY, lineHeight: 1.35 }}>{truncate(app.description || `Install ${app.name} with one command.`, 120)}</div>
          </div>
        </div>
        <div style={{ display: "flex", marginTop: "auto", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 14, padding: "14px 22px", border: `1.5px solid ${BORDER}`, borderRadius: 20, backgroundColor: "#FFFFFF", fontSize: 26, fontWeight: 500 }}>
            <span style={{ color: BLUE, fontWeight: 600 }}>$</span>
            <span>{`silicon-apps install ${app.app_id}`}</span>
          </div>
          <div style={{ display: "flex", gap: 24, fontSize: 24, color: SECONDARY }}>
            {stats.map(stat => (
              <span key={stat}>{stat}</span>
            ))}
          </div>
        </div>
      </div>
    ),
    { width: 1200, height: 630, fonts: await fonts, headers: { "Cache-Control": "public, max-age=3600, stale-while-revalidate=86400" } },
  );
}
