# Organization publishing and catalog rollout

The store, Developer portal and supporting services went live on 17–18 November 2026. The catalog contains all 30 taglines from the supplied Markdown. Briefcase, DM, Ting, Write and Silicon Writings serve the five supplied PNG files. [Silicon Apps](https://apps.teamofsilicons.com) · [Silicon Developer](https://developers.teamofsilicons.com)

## Production components

| Component | Deployed source | Production check |
| --- | --- | --- |
| Apps API and store | `dc350609a5406efc2746f41ac5887406861ede12` | Catalog access, public package resolution, recovery and OpenAPI |
| Accounts and Developer | `6f20755067758a2593ed0cfca7074a6fbdc61051` | Accounts migrations 40–42, catalog sign-in, portal and public documentation |
| Organizations | `cd50cebfc69f455c08f3fd8171ebf40a075d3421` | Service readiness and migration 5 |
| Contacts | `866ffbef7326d001c6381d3382dfe322302be730` | Service readiness and Developer connection route |
| Interfaces | `1fe00b5824753b983a93bf8e2e3a1380cd57a5f6` | Host release `c2f860ef3efc0706bd4f086e419ae865bd8a3d7e8a4ef0534667c9a0a1fbc639`, authenticated organization lookup and migration 8 |
| Cells | `461763eba58aef38cf3806681167321b925482e0` | Host release `81101878e440348416baf1498e795368d626a0be23201d76b76d92acd69a083c` and migrations 38 and 39 |

The Developer portal supports publishing from a personal account or an organization. An organization keeps ownership when a contributor leaves. Its administrators can grant draft, publishing and management permissions by resource type, team or account, and delegate access to an individual app, interface or Cell. Private organization apps start with the whole organization as their audience. Managers can include or exclude named teams, Carbons and Silicons. [Permissions and API examples](../docs/organization-publishing.md)

The people picker combines invitations across the three resource types. It ranks by twice the number sent plus the number received, then recency, then saved contact trust. It displays a contact's saved name and photo when available. The portal can create contacts, set trust and filter shared resources and Cell files. Older files without a recorded owner cannot be attributed to a contact.

## Acceptance

- All 30 production list and detail API records matched the source taglines. All five public image downloads matched the supplied PNGs byte for byte. The Accounts logo projection also matched. Desktop and mobile Chromium displayed the taglines in the store and decoded the five 512-pixel images on the app pages.
- A disposable personal app was created under the designated Silicon identity. Its tagline was edited, its deletion hid it from public reads, its manager restored it, and an ordinary deletion returned it to the recovery list. The recovery period is 30 days.
- The Apps listing registers its own OpenAPI URL. The live specification describes 81 operations across 67 paths. Interfaces can discover operations, authentication and capabilities from the listing and the public guide. [OpenAPI](https://apps.teamofsilicons.com/openapi.json) · [Capabilities](https://apps.teamofsilicons.com/v1/capabilities) · [Interface guide](https://developers.teamofsilicons.com/docs/apps/start/prepare-for-interfaces#1-publish-your-interface-api-documentation)
- Seven catalog reads accept the dedicated `silicon-apps-catalog` audience. A production token for the test account read the store, reviews, target information and a signed resolution for an app the account manages. The management list was empty, the deleted fixture returned 404, and the keys and organization-access endpoints returned 401. The Accounts consent and exchange tests verified that a third-party interface receives the catalog token after approval. The Apps workspace passed 161 tests, and all 24 Accounts interface-auth integration tests passed.
- The portal passed 103 unit tests and 28 Chromium journeys. These included team grants, restricted private audiences, saved contact portraits, ranking, file filters and app recovery. Isolated database tests exercised organization membership changes, grants, credential boundaries, app lifecycle and the Contacts connection.
- Authenticated requests under the designated Silicon identity reached the live organization and people endpoints. That identity had no organization memberships. Cross-member grants, invitation and revocation were exercised in isolated tests. Production portal checks covered the public experience.
- Apps CLI 0.3.5 resolved to the expected signed archives on all nine advertised targets. Fresh public installations passed on Linux x86_64, macOS ARM64 and Windows x86_64. The Rust client is published at the same version. [CLI release](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.3.5) · [Public installation checks](https://github.com/teamofsilicons/silicon-apps/actions/runs/44159996239) · [Rust client](https://crates.io/crates/silicon-apps-client/0.3.5)
- Cells CLI 0.1.1 also passed fresh signed installs on Linux x86_64, macOS ARM64 and Windows x86_64. Each installed CLI exposed the organization option on Cell creation. [Public Cells installation checks](https://github.com/teamofsilicons/silicon-cells/actions/runs/44162149687)
