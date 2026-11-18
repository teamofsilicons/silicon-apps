# Organization publishing, contacts and interface access

The original organization publishing release is recorded in the [production verification](../deploy/verification-organization-publishing-2026-11-18.md).

## Ownership and permissions

Create an app with `POST /v1/apps` and an optional `organization_id`. A work account defaults to its home organization. Send an explicit JSON null to choose a personal publisher. The service checks Create permission before reserving an organization app. `GET /v1/organizations` returns the caller's live organization publishing context. The portal's Publisher selector shows organizations for which the caller has Create permission for that resource kind.

Organization IDs own resources independently of the creator. Author records are attribution, not an authorization fallback. Every organization operation requires current membership, current access to the product and the relevant permission. Removing a member or team grant takes effect on the next request, including idempotency replays. Operations that continue in the background recheck Organizations through service credentials.

Permissions are `create`, `read`, `edit`, `publish` and `manage`. Manage implies Read, Edit and Publish for existing resources; it does not imply Create. Edit and Publish imply Read. Organization owners/admins retain authority. A creator receives Read/Edit on the new resource; Create alone never grants Publish or Manage. Creation responses for organization resources omit server secrets. A manager issues a secret/key through the normal credential controls. Existing personal creation responses are unchanged.

The **Organization** tab opens `/organizations` for organization owners and admins. Its settings apply grants to Apps, Interfaces, Cells, or all three. A grant names a current account UUID or team ID:

```json
{"grants":[{"subject":{"type":"team","id":"team-id"},"resources":["apps","interfaces","cells"],"permissions":["create","edit"]}]}
```

Per-resource policies omit resource kinds and cannot grant Create. Managers can give one person or team access to just one resource without granting access to every organization resource. Apps use `GET/PUT /v1/apps/{app_id}/organization-access`. Interfaces use `PUT /v1/interfaces/{id}/organization-access` with `{ "revision": 4, "access": POLICY }`. Cells use `GET/PUT /v1/cells/{id}/organization-access` with the policy directly. Mutations use their service's `Idempotency-Key` rules.

```json
{"grants":[{"subject":{"type":"account","id":"account-uuid"},"permissions":["read","edit"]}],"audience":{"mode":"restricted","include":[{"type":"team","id":"team-id"}],"exclude":[{"type":"account","id":"another-account-uuid"}]}}
```

Private organization apps default to the whole organization. A restricted audience includes selected people/teams. Exclusions win over audience inclusion. Resource managers/readers retain their separately granted management access. Publishing permission is separate from audience access. Apps enforce this for listing, details, packages, downloads, storage and event visibility. Interfaces apply their audience to published listings and uploads. Cells management grants do not replace database RLS, linked-app user authentication or object-storage rules.


### Invite an organization as an author

In Developer, open the app's **Authors** tab. Under **Organization authors**, enter the organization ID and choose **Invite organization**. An account with the recipient organization's Apps Manage permission can accept in **Invitations**. The API takes `{"to":"org:studio"}` on `POST /v1/apps/{app_id}/invites` and uses the regular accept or decline endpoint.

Acceptance records the organization ID. Its members contribute using their current Apps Read, Edit and Publish grants. The owning publisher retains management authority, including deletion and author removal. A host manager can remove the organization in **Authors** or with `DELETE /v1/apps/{app_id}/organization-authors/{org_id}`. The publisher of an existing app stays fixed.

## People and Contacts

`GET /v1/people` on Apps, Interfaces and Cells returns invitation signals for the signed-in account: peer UUID/profile, `sent`, `received`, `last_invited_at`. The portal merges these, overlays the owner's saved Contacts names/photos and ranks by `2 * sent + received`, then latest invitation, then contact trust. A fuzzy filter preserves that order. Blocked contacts are excluded. Counts come from committed invitation history, not retries.

The portal's Contacts page connects the existing Silicon Contacts address book explicitly. Names, photos, trust (1–255) and other saved contact fields remain the same data as in Silicon Contacts. Connecting permits contact reads/writes, not messages or relationship/settings changes. Disconnecting immediately denies further address-book reads through the developer connection; it does not delete contacts. Exact public account lookup can preview a Carbon/Silicon without connecting the address book.

Resource lists filter by creator, Shared with me, saved contacts, selected person and organization publisher. Cell file filters operate only on files already authorized by that Cell: `owner_ids` (comma-separated UUIDs) and `shared_with_me`. For a large contact list use `POST /v1/cells/{id}/storage-api/files/{bucket}/query` with JSON `{ "owner_ids": "uuid1,uuid2", "shared_with_me": true, "prefix": "", "limit": 100, "after": "last-name" }`. It is a read operation, preserves RLS and accepts up to 20,000 owner IDs. The GET file-list endpoint accepts the same query fields. New author uploads record their owner UUID; historical files without an owner are not guessed to belong to someone.

## Delete and restore apps

`DELETE /v1/apps/{app_id}` requires personal administration or organization Manage. Open **Apps → your app → Settings**, type the app ID and choose **Delete app**. The **Deleted apps** section on the Apps home page offers restoration. Deletion hides discovery/distribution, denies app user storage and disables the Accounts app. Pending invitations are cancelled. `GET /v1/apps?mine=true&deleted=true` lists recoverable/deleted resources visible to the current manager/reader. `POST /v1/apps/{app_id}/restore` restores within 30 days, with the same current authorization checks. Publication state is retained. IDs remain permanently reserved. This is recoverable deletion, not immediate physical erasure of historical records/backups. The platform's own Accounts, Apps and Developer registrations cannot be deleted here.

The CLI exposes `delete`, `restore`, `list --mine --deleted`, `organizations`, `organization-access APP [--file POLICY.json]`, and `create --organization ID`. Client methods mirror these operations.

## Integrating an interface

Use `/openapi.json`, `/v1/capabilities` and `/.well-known/agent-card.json` to discover the Apps API. Add `silicon-apps-catalog` to an interface's app IDs. Send the Accounts token from `apps["silicon-apps-catalog"].access_token` as `Authorization: Bearer TOKEN`. The service verifies the signature, audience and live sign-in, then applies the account's current sharing rules to published apps. The `interface_catalog` capability lists the supported reads.

Interfaces can search the catalog, display a listing and its API documentation, use cases, license, reviews and media, and resolve or download signed releases. Guarded app storage uses separate account and app proofs. Publishing, recovery, event streams and webhooks use the first-party Apps or Developer permissions documented in OpenAPI. Organization webhooks recheck authority for the target app before delivery.

New publication requires a one-line tagline (maximum 60 characters) and an HTTPS interface API documentation URL. Existing publications are grandfathered. Listing metadata describes the app's actual API; it does not generate arbitrary endpoints for third-party apps.

## Rollout and local verification

Apply Accounts migrations `0040_organization_publishers.sql` and `0041_app_registry_lifecycle.sql`, and deploy Organizations' publishing/service-context routes first. The organization-author update also needs the exact publisher lookup in Organizations and Cells migration `0040_organization_authors.sql`. Then deploy Apps, Interfaces (migration `0008_resource_creator.sql`) and Cells (migrations `0038_organization_publishers.sql` and `0039_resource_creator.sql`) against that Organizations URL; deploy Contacts' `0004_developer_connection.sql` migration and API before the portal. Keep every vendored `silicon-publishing-policy` identical to the Organizations source. Apps/Cells/Interfaces service credentials are checked by Accounts for background authorization. Do not copy user sessions or service secrets into browser configuration.

Environment settings: `APPS_ORGANIZATIONS_URL`, `ACCOUNTS_ORGANIZATIONS_URL`, `INTERFACES_ORGANIZATIONS_URL`, `CELLS_ORGANIZATIONS_URL`, and the portal's `ORGANIZATIONS_API_URL` / `CONTACTS_API_URL`. All live context fetches fail closed for organization resources. Automatic app/interface Cell creation additionally requires Create and Manage on Cells because it configures credentials and links.

Tests cover the shared evaluator, live team removal, departed members, cross-audience/service credential rejection, durable Accounts ownership, draft credential redaction, private visibility, scoped Contacts connect/disconnect, deletion/replay behavior and desktop/mobile portal flows. See the repositories' test commands and the developer portal E2E specification `organizations-contacts-delete.spec.ts`.
