# Arc UI provenance

The source under `src/components/arc/` was downloaded verbatim from the public Arc UI free registry on 2026-10-08. It is MIT licensed, copyright 2026 Elia Kuratli. The accompanying LICENSE is retained here. No Pro source or imitation of Pro components is included.

Source index: https://uiarc.dev/llms.txt
License: https://uiarc.dev/license
Registry: https://uiarc.dev/r/registry.json

Items: `arc-foundation`, `arc-motion-tokens`, `button`, `input`, `textarea`, `search-field`, `badge`, `empty-state`, `dialog`, `switch`, `copy-button`, `stepper`, and `segmented-control`.

Each item came from `https://uiarc.dev/r/<item>.json`. Its complete registry response is retained beside this file; corresponding component guidance lives under `docs/`. Registry installation uses `scripts/install-arc.py`, which performs the documented manual source-copy installation, follows free registry dependencies, and preserves all source files.

The application imports Arc foundation once and uses its button, input, textarea, search field, badge, empty state, Radix-backed dialog, switch, copy button, and segmented control directly. App-specific screens compose those components. The setup navigation is application-specific because the required freely traversable wizard differs from Arc Stepper's sequential-only navigation.

Fonts are bundled through `@fontsource/inter` and `@fontsource/geist`, retaining their package licenses.

The theme-switch component and its light/dark/system theme behavior are reused from the Silicon Accounts developer portal on 2026-10-09. They retain the same Arc button and motion tokens; store persistence uses `apps.theme`. The theme transition CSS and author surfaces adapt the shared Silicon design tokens.
