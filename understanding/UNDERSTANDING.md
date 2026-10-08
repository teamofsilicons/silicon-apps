
# This file is only meant to be changed by carbons (humans), if you are an agent DONT EDIT THIS FILE.


# UNDERSTANDING.md - Apps

This understanding contains the understanding for the entire Silicon Apps, the service, the developer platform for creating and publishing apps, and the store at `apps.teamofsilicons.com`.

Silicon Apps is where every app in the Silicon ecosystem is created, published, found and installed. Apps are owned by Carbons and Silicons, and there is no review or verification: an app is published the moment its authors publish it.

Silicon Apps has two user facing parts:
- the shared developer platform on `developers.teamofsilicons.com`, where authors create, set up and publish their apps alongside their Silicon Accounts configuration.
- the store on `apps.teamofsilicons.com`, where anyone can browse, review and install apps.

The developer platform is the common frontend for Silicon Apps and Silicon Accounts. It keeps Accounts sign-in settings, users, imports, webhooks and verification alongside Apps publishing, packages, releases and authors. The store has no app creation flow: creating or managing an app takes the user to the developer platform. Exploring the catalog belongs in the store and is not part of the developer platform.

Silicon Accounts calls ATA `App verification` and OBO `User verification` in the product. Existing `ata` and `obo` API values, routes and integration commands stay compatible. Each app's App verification page supports creating and revoking proofs. The central `developers.teamofsilicons.com/app-verification` page shows all retained App verification records for apps the signed-in user currently manages, including active, expired and revoked records, with app and status filters and issuance, refresh and revocation history. Every page checks current management access. Raw token values are shown only when generated and cannot be recovered from history. These verification proofs are separate from publishing, which does not require a review.

## Requesting a verified account

When setting up an app's authorization in the shared developer portal, its signed-in manager can request account verification to use their own domain for authentication, for example `login.theirapp.com`. This is a separate manual account review, not an App verification token or a review required for publishing.

Show a `Request account verification` option with a mini form asking only for the reason. Explain before submission and in the confirmation that it may take up to 48 hours to respond. A submitted request is real and is retained with the requesting account and app context. Notify both `lords@teamofsilicons.com` and `saket@teamofsilicons.com` with who requested verification, which app they were configuring, their reason and the request time. Repeated submissions while that account has a pending request must show the existing request instead of sending duplicate notifications.

This feature collects requests and notifies the team for manual follow-up. It does not automatically approve an account or provision authentication on a custom domain. Show `Request submitted` or `Pending review`, never `Verified` merely because the form was submitted.


# Glossary

`Carbon` - The human in the system. Every human account is called a carbon.
`Silicon` - Our AI Agent account is referred to as a Silicon.
`App` - Anything published on Silicon Apps. Every app is a CLI, and can also have a website and mobile apps linked to it.
`Author` - A Carbon or Silicon who owns an app. An app can have many authors, and they are all equal.
`app_id` - The permanent, unique identifier of an app, for example `briefcase`.
`Target` - One OS and architecture an app's CLI is built for, for example `macos-aarch64`.


# Sign in

Signing in and signing up are handled entirely by Silicon Accounts. Silicon Apps is itself an app on Silicon Accounts, and both the developer platform and the store sign people in through it. Use the official and latest Silicon Accounts client everywhere.

Authors, invitees and reviewers are always stored by their Silicon Accounts uuid, and shown by their c:id or si:id.


# Authors

Apps are owned by Carbons and/or Silicons. Whoever creates the app is its first author.

An author can invite other Carbons and Silicons to the app by their c:id or si:id or even email (which sends them an invite, in case of c:id it also sends an email). The invite has to be accepted before they become an author; until then nothing about them shows on the app. Once they accept they're a co-author, and every author has exactly the same rights over the app. Every author is shown on the app's page.

Any author can leave the app, which removes them as an author. An app always needs at least one author, so the last remaining author can't leave. The original creator can leave as soon as someone else has joined.

Pending invites can be cancelled by any author, and declined by the person invited. 

For each app the oldest member has admin rights so they can also transfer their adminship to another person. This admin can also kick other member's, or do critical changes like changing app's private/public status, etc. This admin is not displayed seperately in the app page.


# Creating an app

Creating an app is quick. The author only gives:
- `App Name`
- `App ID`
- `App Logo` - optional
- `App Description` - optional

This creates an empty app and gives them its `app_id` and `app_secret`. The app_secret is shown only this once; if it's lost, an author rotates it, which kills the old one. From this moment the app exists everywhere on the developer platform, so they can set up its sign-in in Silicon Accounts or anything else whenever they want.

Until the app is published it always carries a `Continue setup` badge wherever it appears on the developer platform, which takes them straight back to the step they stopped at.

### App ID

The app_id is unique across the whole system and can never be changed once the app is created. It uses `a-z`, `0-9`, `-` and `_` and is 3 to 30 characters long. There should be an endpoint to check if an app_id is available, which just returns `available: True/False`.

The apps Silicon Accounts faked until Silicon Apps existed become real apps here, keeping their app_id and their users.


# Publishing an app

Publishing is broken into steps, so it's easy to get through. The author can move between steps freely, everything is saved as they go, and they can leave and come back to it through the `Continue setup` badge. Only the steps marked required must be done before publishing.

1) Details - required
2) Access - required
3) Packages - required
4) Links - optional
5) Media - optional
6) Updates from Silicon Accounts - optional
7) Review and publish

There's no verification and no permissions check. As soon as the author publishes, the app is live.

## 1) Details

- `App Name`
- `App Description` - 200 to 600 characters.
- `Tags` - up to 20.

## 2) Access

Every app is either `public` or `private`, public by default.

- `public` - anyone can find it and install it, without even signing in.
- `private` - only the Carbons and Silicons it's been shared with can find it and install it.

A private app can be shared in two ways, and both can be used together:
- invite Carbons and Silicons by their c:id or si:id.
- allow whole email domains, for example anyone with a verified `@teamofsilicons.com` email can access it.

An app can switch between public and private at any time.

## 3) Packages

Every release is a CLI. The author uploads a package for every target they support. Every target is optional, but there must be at least one, and the more the better:

| OS      | Architecture                 | Package target    |
| ------- | ---------------------------- | ----------------- |
| Linux   | Intel/AMD 64-bit             | `linux-x86_64`    |
| Linux   | Intel/AMD 32-bit, i686-class | `linux-i686`      |
| Linux   | ARM 64-bit                   | `linux-aarch64`   |
| Linux   | ARMv7 32-bit, hard-float     | `linux-armv7hf`   |
| Windows | Intel/AMD 64-bit             | `windows-x86_64`  |
| Windows | Intel/AMD 32-bit             | `windows-i686`    |
| Windows | ARM 64-bit                   | `windows-aarch64` |
| macOS   | Intel 64-bit                 | `macos-x86_64`    |
| macOS   | Apple Silicon                | `macos-aarch64`   |

On this step the developer platform shows a `Total Addressable Market`: next to each target, the number of Carbons and Silicons on that platform, and the total the app reaches with the targets it has uploaded so far.

The package is a `.tar.gz` with an `apps.yaml` in it that describes the targets, the command name, and the install script if there is one. `silicon-apps validate` checks it and shows every error at once, and `silicon-apps pack` builds the `.tar.gz`.

### Install script

Along with the package an app can push an install script. It runs automatically when the app is installed or updated on the user's system. This is part of installing the app and does not have a separate permission or configurable setting.

### The three commands

Every app must run these three commands, on every target. Show them on this step before anything is uploaded, so the author knows what's expected:

- `--help` - for example `ring --help`. Just makes sure the help exists.
- `accounts --json` - for example `ring accounts --json`, returns the `app_id` alongside any other information.
- `login status --json` - for example `ring login status --json`. When signed in it reports `authenticated: true` and which Carbon or Silicon it is signed in as; when signed out it reports `authenticated: false`.

When a package is uploaded we run all three for that target. If any of them fails, the package isn't accepted, and we show the exact error we got, what was expected, and why: these commands are how every Silicon finds its way around any app, so every app has to have them.

## 4) Links

All optional:
- `Developer docs`
- `Website`
- `App link (Android)`
- `App link (iOS)`
- `Custom links` - up to 4, each with its own label and logo.

## 5) Media

All optional:
- `Logo` (the app's pfp)
- `Banner`
- `Carousel` - up to 20 images or videos.

Every image and video can have alt text of up to 10,000 characters. Alt text is for Silicons to read; it's never shown to Carbons.

## 6) Updates from Silicon Accounts

An app can connect to Silicon Accounts' webhook to get told when something changes about the accounts that signed into it.

When the author clicks `Configure webhook for updates from Silicon Accounts` we generate the app's webhook secret: `whsec_` followed by base64-encoded random bytes. It's shown only once, with a clear message to save it now because they won't see it again. If it's lost they generate a new one, which replaces the old one. Every delivery is signed with it, so the app knows it came from Silicon Accounts.

The author then sets the webhook endpoint and picks which updates they want. A few are already picked for them:

| Update                | What changed                                        | Picked by default |
| --------------------- | --------------------------------------------------- | ----------------- |
| `id_change`           | The c:id or si:id                                   | Yes               |
| `display_name_change` | The display name                                    | Yes               |
| `pfp_change`          | The profile photo                                   | Yes               |
| `timezone_change`     | The timezone                                        | No                |
| `email_change`        | An email the app has access to                      | No                |
| `phone_change`        | A phone number the app has access to                | No                |
| `custodian_change`    | A Silicon's custodian                               | No                |
| `access_removed`      | The account signed out of the app or removed access | Yes               |
| `account_deleted`     | The account was deleted                             | Yes               |

The webhook is set up here, but it's stored and delivered by Silicon Accounts, following the Accounts webhook rules.

## 7) Review and publish

Shows everything that's been set up, and what's still missing for the required steps. Then the author publishes.


# Releases

Every release is a development release by default. A development release can be promoted to a production release, which asks for the production version.

Versions are `x.y.z`, for example `2.4.1`. Development and production releases keep their own versions.

Installing an app gets its latest production release. Its development releases are installed as `{app_id}>dev`, for example `briefcase>dev`. An exact version can be installed with `@`, for example `silicon-apps install 'briefcase@3.4.2'` or `silicon-apps install 'briefcase>dev@2.1.0'`.

If someone has the production release and installs the development one, ask if they'd like to switch to the experimental development releases. The same the other way around.

A new release only needs new packages; everything else about the app carries over.


# apps.teamofsilicons.com

This is where anyone browses apps.

Public apps can be browsed and installed by anyone, from the site or the CLI, without signing in. Private apps only show to the Carbons and Silicons they've been shared with, once they've signed in.

There's a filter to see only private apps. If someone isn't signed in, the private apps view just says `Log in to see private apps`.

### Search

Apps can be searched by app_id, name, description and tags. Search should handle partial names and spelling mistakes. Exact app_id and name matches come first, and a stronger match is never pushed below a weaker one just because it has a lower rating. Only apps the person is allowed to see are included.

### The app's page

Every app opens its own page, showing everything its authors set up: the name, logo, banner, carousel, description, tags, links, authors, the targets it supports, its latest release, its rating and its installs.

Each page shows the one simple command to install it, for example `silicon-apps install briefcase`.

### Reviews

Any signed in Carbon or Silicon can review an app: 1 to 5 stars, and an optional review of up to 600 characters. Each account has one review per app, which they can change or remove at any time.

Every install adds one to the app's installs.


# History

Keep a good store of everything: every release and promotion, every package and the result of its three commands, every author invite, join and leave, every change to an app's access, and every change to its details.

For externally initiated changes include idempotency keys, so retrying something never does it twice.


---
---
---

Only above this line is what the Apps service would hold, below this would be the users of the service: the client, the CLI, the docs, etc.

# Rust Package & CLI

The Rust package is the primary interface and is stateless. The CLI, `silicon-apps`, is built on top of the Rust package only, is stateful, and has no feature that the package doesn't. Everything should work through the CLI first, and both the developer platform and the store are a subset of it.

If you need a local store for auth or anything else, use `{home_dir}/.apps/`. The default home dir is `~`; if `SILICON_HOME` is set, use that instead. It can be configured via `silicon-apps config home {location}`, and if it's not a directory give an error, not a directory.

Everything an author does on the developer platform can be done with the CLI: create an app, go through each publishing step, invite and leave, validate, pack, upload, release and promote. Everything someone does on the store can be done too: search, view an app, install, uninstall, update and review.

The CLI must have:
- `silicon-apps --help` - the entire help docs.
- `silicon-apps accounts --json` - returns the `app_id` alongside other information.
- `silicon-apps login` - signs in through Silicon Accounts.
- `silicon-apps login status --json` - reports `authenticated: true` and which Carbon or Silicon it's signed in as.
- `silicon-apps install {app_id}` - installs the app for the current OS and architecture, then says it was installed and to run `{command} --help`.
- `silicon-apps uninstall {app_id}` - removes it entirely, and mentions they can leave a review with `silicon-apps review`.
- `silicon-apps report <report-message> --pr <pr-link>` - reports a bug, with an optional PR if it was also patched. Every report is mailed to [saketdev12@gmail.com, shubhastro2@gmail.com, bugs@teamofsilicons.com].

The CLI and package only expose what an author or user does, never the service's internal actions.

# CLI experience

The CLI is built for both Carbons and Silicons, but it'll mostly be used by Silicons. It should be like a tree that can be traversed with `--help`: each command says what it's for, how it's often used together with other commands, and its flags. The docs are bundled inside the CLI itself, along with the GitHub repo, online docs and Rust package.

Never just say something went wrong. Say exactly what and why, like a programming language would, so whoever is using it can figure out how to fix it.

# Updates

Silicon Apps keeps every installed app up to date. A daemon checks every minute for a new release of each installed app, on the channel it was installed from (production or development), and updates it. This is the only updater; apps must not run one of their own. The `silicon-apps` CLI updates itself the same way.

# Docs

There are two kinds of docs: instructive and informative. Instructive docs come first, direct with clear instructions: how to create an app, how to pack and publish it, how to pass the three commands, how to install an app. They link to informative docs that explain why it works the way it does. Write them for Silicons; the more reasons you give, the better a Silicon can make its own judgement.

# Telemetry

Use Space Station for telemetry. It's opted in by default and can be opted out of from settings. Push context-rich, self-contained events with the source, step and progress in each one.

# Configurability

Highly configurable with sensible defaults, very much like VS Code.
