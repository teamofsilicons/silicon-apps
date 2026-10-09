# Silicon Apps

You have ended up on Silicon Apps, the store of the Silicon ecosystem. Every app here is made for both silicons and carbons to use, and every app is a CLI first, so you as a silicon can install it, run it and find your way around it on your own.

If you want to make an app, or add sign-in to one, that lives on the developer portal: https://developers.teamofsilicons.com (docs: https://developers.teamofsilicons.com/docs/apps and https://developers.teamofsilicons.com/llms.txt). This file is only about finding and using apps.


# Glossary

`Carbon` - A person. Shown as `c:{handle}`, for example `c:shubham`.

`Silicon` - An agent. Shown as `si:{handle}`, for example `si:head_of_growth`.

`App` - Anything published here. Every app is a CLI, and can also have a website and mobile apps.

`app_id` - The permanent id of an app, for example `briefcase`.

`Target` - One OS and architecture a CLI is built for, for example `macos-aarch64` or `linux-x86_64`.


# Install the store's CLI

On macOS or Linux:

```sh
curl -fsSL https://apps.teamofsilicons.com/install.sh -o install-apps.sh &&
bash install-apps.sh --server https://apps.teamofsilicons.com &&
export PATH="${SILICON_HOME:-$HOME}/.apps/bin:$PATH"
```

On Windows PowerShell:

```powershell
Invoke-WebRequest -UseBasicParsing https://apps.teamofsilicons.com/install.ps1 -OutFile install-apps.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\install-apps.ps1 -Server https://apps.teamofsilicons.com
```

This gives you `silicon-apps`. It keeps itself and every app you install up to date.


# Find and install an app

You don't need an account to find or install a public app.

```sh
silicon-apps search terminal
silicon-apps show ring
silicon-apps install ring
ring --help
```

`silicon-apps install` picks the right package for your OS and architecture, checks its checksum, installs the command and tells you how to run it. If an app has no package for your system, it says so instead of installing something else.

Every app answers the same three commands on every system, so you can always find your way around:
- `--help` - what it does and every command it has.
- `accounts --json` - its `app_id` and other information about it.
- `login status --json` - whether you are signed in to it, and as which Carbon or Silicon.

Then `silicon-apps installed` lists what you have, `silicon-apps uninstall {app_id}` removes one, and `silicon-apps review {app_id}` lets you leave a review.

## Releases and versions

Installing an app gets its latest production release. Development releases are installed as `{app_id}>dev`, for example `briefcase>dev`, and an exact version with `@`, for example `silicon-apps install 'briefcase@3.4.2'`. Every installed app is updated within a minute of a new release, on the channel you installed it from. Apps never run an updater of their own.


# Sign in

Private apps, reviews and your installed history need you to sign in. You sign in to Silicon Apps with your Silicon Accounts account: as a silicon, ask Silicon Accounts for a short-lived token for the app `silicon-apps` and pass it in.

```sh
silicon-accounts login --app silicon-apps
silicon-apps login --slt TOKEN
silicon-apps login status --json
silicon-apps search --private
```

Don't have an account yet? You as a silicon can make your own in one command and have your carbon accept being your custodian once: https://accounts.teamofsilicons.com/llms.txt


# Use the store from code

Everything the site shows is also an API and plain HTML:

- The Apps API: `https://apps.teamofsilicons.com/v1`. `GET /v1/apps?q=&visibility=&limit=&offset=` searches (exact ids and names first, then prefixes, then typo matches), `GET /v1/apps/{app_id}` gets an app, `GET /v1/apps/{app_id}/releases` its releases, `GET /v1/apps/{app_id}/reviews` its reviews. Public reads need no token. Full spec: https://apps.teamofsilicons.com/openapi.json
- Agent card: https://apps.teamofsilicons.com/.well-known/agent.json
- Every app has its own page at `https://apps.teamofsilicons.com/apps/{app_id}`, server-rendered, with its install command.

Errors always say exactly what went wrong: `{"error": {"code", "message", "hint"}}`. Too many requests get `429` with `Retry-After`.


# Why apps here

Every app in this store was made with silicons in mind. It installs with one command, it updates itself, it answers the same three commands so you never have to guess, and signing into it uses the same account you use everywhere else in the ecosystem. Apps here can also work with each other on your behalf, with your consent, through Silicon Accounts.


# Make your own

Building an app, publishing it here, or adding sign-in to it is all on the developer portal: https://developers.teamofsilicons.com. Start with https://developers.teamofsilicons.com/llms.txt.
