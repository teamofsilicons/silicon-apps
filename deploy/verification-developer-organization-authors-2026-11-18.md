# Developer organization authors and resource settings

The 18 November 2026 production release enables organization author invitations for Apps, Interfaces and Cells. Work accounts with Create permission default to their own organization as publisher. Organization owners and admins can edit account and team publishing permissions in the Developer portal. App and Interface Settings accept the resource ID as deletion confirmation. Managers can restore a resource from its home page for 30 days. An accepted author invitation leaves the existing publisher in control.

## Production releases

| Component | Source commit | Active release |
| --- | --- | --- |
| Developer and Accounts | `69b0a49f5eb207f3a60a50471430f1ae01342cf8` | `69b0a49f5eb207f3a60a50471430f1ae01342cf8` |
| Apps API and store | `154b56b9bceaf633f554eb3a8c7267db6022d9eb` | `154b56b9bceaf633f554eb3a8c7267db6022d9eb` |
| Interfaces API and catalog | `ae2e99dd4f6f356ae74a3ddb77ae791140858e48` | `0ed913d960bf9237452a10a79d6cf39e514372e1864b643df44b7dfb3dab9564` |
| Organizations API | `16fcf25ce2e80703ba8de6dfa9bab3295041b43b` | `16fcf25ce2e80703ba8de6dfa9bab3295041b43b` |
| Cells API | `dced18fabbb9b233c99fa294fd8a6a6b3b9d7d13` | `50eba076e689626570af35a095bdf8c7e6dcf587a1734081cb41cc1cc8643b0e` |

The release operator verified the running executable and readiness on each host. The Cells operator backed up the control database, applied migration 40 in a transaction, and cleared the maintenance fence. Production checks confirmed that all 40 migration entries succeeded, the author organization array exists, and both new indexes are present.

## Verification

- The shared Silicon identity signed in to the production Apps, Interfaces and Cells APIs. Each returned its publishing permissions and invitation inbox. Each product session resolved an existing organization by its exact ID and received HTTP 404 for an unknown ID. Interfaces also returned the authenticated deleted-resource list.
- A signed-out Chromium session opened the five management entry points, the Apps and Interfaces guides and the sign-in page. All eight pages loaded with zero uncaught browser errors or failed static assets. Checksums matched the production JavaScript for invitations, organization settings, app deletion and interface restoration.
- The public Apps store and contract, Interfaces catalog and API, Cells API, Organizations and Accounts returned HTTP 200.
- Locally, all 121 portal browser tests and 103 unit tests passed. Isolated service tests exercised organization invitation acceptance and removal, current member permissions, hosting defaults, and typed deletion and restoration. A PostgreSQL 16 upgrade from schema 39 preserved an existing Cell and rejected a repeat submission.
- The production verification identity had no organization memberships. Cross-organization acceptance, work-account defaults and privileged deletion and restoration were exercised with isolated accounts and resources.

[Developer portal](https://developers.teamofsilicons.com) · [Organization publishing guide](../docs/organization-publishing.md)
