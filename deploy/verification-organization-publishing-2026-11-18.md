# Organization publishing and catalog rollout

The store, Developer portal and supporting services went live on 17–18 November 2026. The catalog contains all 30 taglines from the supplied Markdown. Briefcase, DM, Ting, Write and Silicon Writings serve the five supplied PNG files. [Silicon Apps](https://apps.teamofsilicons.com) · [Silicon Developer](https://developers.teamofsilicons.com)

## Production components

| Component | Deployed source | Production check |
| --- | --- | --- |
| Apps API and store | `41be4ee98fc21a043bbf890b1c1ff12f9c07c3eb` | Catalog, public package resolution, recovery and OpenAPI |
| Accounts and Developer | `7603e1496961338136898729e9da0ea157ad5aae` | Accounts migrations 40 and 41, portal and public documentation |
| Organizations | `cd50cebfc69f455c08f3fd8171ebf40a075d3421` | Service readiness and migration 5 |
| Contacts | `866ffbef7326d001c6381d3382dfe322302be730` | Service readiness and Developer connection route |
| Interfaces | `81e0c18e4a774bb820aa18ce5056842c91fe7241` | Host release `7f4486ca51f6c6f054e8f42dd76943b65b669714ca4c0e3106872236abd60605` and migration 8 |
| Cells | `461763eba58aef38cf3806681167321b925482e0` | Host release `81101878e440348416baf1498e795368d626a0be23201d76b76d92acd69a083c` and migrations 38 and 39 |

The Developer portal supports publishing from a personal account or an organization. An organization keeps ownership when a contributor leaves. Its administrators can grant draft, publishing and management permissions by resource type, team or account, and delegate access to an individual app, interface or Cell. Private organization apps start with the whole organization as their audience. Managers can include or exclude named teams, Carbons and Silicons. [Permissions and API examples](../docs/organization-publishing.md)

The people picker combines invitations across the three resource types. It ranks by twice the number sent plus the number received, then recency, then saved contact trust. It displays a contact's saved name and photo when available. The portal can create contacts, set trust and filter shared resources and Cell files. Older files without a recorded owner cannot be attributed to a contact.

## Acceptance

- All 30 production list and detail API records matched the source taglines. All five public image downloads matched the supplied PNGs byte for byte. The Accounts logo projection also matched. Desktop and mobile Chromium displayed the taglines in the store and decoded the five 512-pixel images on the app pages.
- A disposable personal app was created under the designated Silicon identity. Its tagline was edited, its deletion hid it from public reads, its manager restored it, and an ordinary deletion returned it to the recovery list. The recovery period is 30 days.
- The Apps listing registers its own OpenAPI URL. The live specification describes 81 operations across 67 paths. Interfaces can discover operations, authentication and capabilities from the listing and the public guide. [OpenAPI](https://apps.teamofsilicons.com/openapi.json) · [Capabilities](https://apps.teamofsilicons.com/v1/capabilities) · [Interface guide](https://developers.teamofsilicons.com/docs/apps/start/prepare-for-interfaces#1-publish-your-interface-api-documentation)
- The portal passed 103 unit tests and 28 Chromium journeys. These included team grants, restricted private audiences, saved contact portraits, ranking, file filters and app recovery. Isolated database tests exercised organization membership changes, grants, credential boundaries, app lifecycle and the Contacts connection.
- Authenticated requests under the designated Silicon identity reached the live organization and people endpoints. That identity had no organization memberships. Cross-member grants, invitation and revocation were exercised in isolated tests. Production portal checks covered the public experience.
- Apps CLI 0.3.5 resolved to the expected signed archives on all nine advertised targets. Fresh public installations passed on Linux x86_64, macOS ARM64 and Windows x86_64. The Rust client is published at the same version. [CLI release](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.3.5) · [Public installation checks](https://github.com/teamofsilicons/silicon-apps/actions/runs/44159996239) · [Rust client](https://crates.io/crates/silicon-apps-client/0.3.5)
