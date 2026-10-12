/**
 * Renders the browser-tab icon into public/ from favicon.svg, with Playwright's Chromium:
 *   favicon.svg → public/icon.svg (as is) and public/favicon.ico (16, 32 and 48 px)
 * The seven hexagons alone with no tile, in ink (#1E1E1C) when the browser is light and cream (#EDE8E0) when it is
 * dark; favicon.ico draws the ink. The install icons (icon-192/512, maskable, apple-touch-icon) keep the cream square. Run it after changing
 * favicon.svg: node scripts/brand/render.mjs
 */
import { readFileSync, writeFileSync, copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "..", "public");
const browser = await chromium.launch();
const svg = readFileSync(join(here, "favicon.svg"), "utf8");
const render = async size => {
  const page = await browser.newPage({ viewport: { width: size, height: size } });
  await page.setContent(`<body style="margin:0;background:transparent"><img style="display:block;width:${size}px;height:${size}px" src="data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}"></body>`);
  const png = await page.screenshot({ omitBackground: true });
  await page.close();
  return png;
};

// favicon.ico: PNG images in an ICO container (a 6-byte header, one 16-byte entry per image, then the images).
const sizes = [16, 32, 48];
const images = [];
for (const size of sizes) images.push(await render(size));
const header = Buffer.alloc(6);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(sizes.length, 4);
let offset = 6 + 16 * sizes.length;
const entries = sizes.map((size, index) => {
  const entry = Buffer.alloc(16);
  entry.writeUInt8(size, 0);
  entry.writeUInt8(size, 1);
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(images[index].length, 8);
  entry.writeUInt32LE(offset, 12);
  offset += images[index].length;
  return entry;
});
writeFileSync(join(out, "favicon.ico"), Buffer.concat([header, ...entries, ...images]));
copyFileSync(join(here, "favicon.svg"), join(out, "icon.svg"));
await browser.close();
console.log("brand: wrote icon.svg and favicon.ico");
