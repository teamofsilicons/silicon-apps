import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";
import siliconUi from "@teamofsilicons/silicon-ui/eslint";

// The account site's dev tools (development only, never in a production page) use Button's soft variant and xs size and
// SegmentedControl's xs size, experimental in Silicon UI 0.4, as the account site allows them.
const DEV_TOOLS = ["components/dev/**"];

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    // Internal links are plain <a> on purpose: every navigation is a full, server-rendered document (fresh header,
    // fresh CSP nonce, works with no JavaScript), as close to plain HTML as the store can be.
    rules: { "@next/next/no-html-link-for-pages": "off" },
  },
  globalIgnores([".next/**", "out/**", "next-env.d.ts", "public/**", "test-results/**", "playwright-report/**", ".screens/**"]),
  // Silicon UI's guardrails, as on the account site: no raw <button>, <input>, <select>, <textarea> or <dialog>, no
  // borrowed component CSS, no local component copies (CSS tokens are checked by silicon-ui-check in `pnpm lint`).
  // nav-list: the sidebar (the account site's, experimental in 0.4). button-link: its soft variant for the sidebar's
  // chips, as the account site and Silicon Starter have them.
  ...siliconUi({ allowExperimental: ["nav-list", "button-link"], ignores: DEV_TOOLS }),
  ...siliconUi({ allowExperimental: ["nav-list", "button", "button-link", "segmented-control"], files: DEV_TOOLS }),
]);
