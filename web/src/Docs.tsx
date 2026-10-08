import { developerUrl } from "./portal";
import { ArrowUpRight } from "lucide-react";
import { Command, PageTitle, Section } from "./ui";

export function Docs() {
  return (
    <>
      <PageTitle
        title="A good place to start"
        description="Follow a task from the first command to a published app."
      />
      <div className="docs-layout">
        <Section title="Install the Apps CLI">
          <p>
            Download the installer for your system, review it, then run it. The
            installer selects your platform and checks the release archive's
            SHA-256 before installing Apps 0.1.4.
          </p>
          <h3>macOS and Linux</h3>
          <Command value="curl -fsSL https://apps.teamofsilicons.com/install.sh -o install-apps.sh" />
          <Command value="bash install-apps.sh --version 0.1.4 --server https://apps.teamofsilicons.com" />
          <h3>Windows PowerShell</h3>
          <Command value="Invoke-WebRequest -Uri https://apps.teamofsilicons.com/install.ps1 -OutFile install-apps.ps1" />
          <Command
            value={
              "powershell -NoProfile -ExecutionPolicy Bypass -File .\\install-apps.ps1 -Version 0.1.4 -Server https://apps.teamofsilicons.com"
            }
          />
          <p>
            The PowerShell policy option applies only to this installer process.
            Add the directory printed by the installer to PATH before using
            <code> apps</code>. Installation starts the updater and registers it
            to run after login. Use <code>--no-startup</code> on macOS/Linux or
            <code> -NoStartup</code> on Windows to skip startup registration.
          </p>
          <h3>Build with Cargo</h3>
          <Command value="cargo install silicon-apps-cli --version 0.1.4 --locked" />
          <p>
            Cargo installs the standalone CLI. The installers above also
            register Apps itself for automatic updates.
          </p>
          <p>
            The initial Apps catalog release for Apps itself supports Linux x64.
            On other platforms, use the installers above;{" "}
            <code>apps install apps</code> cannot yet find a matching catalog
            package. GitHub downloads are available for all nine supported
            platforms.
          </p>
          <a
            href="https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.1.4"
            target="_blank"
            rel="noreferrer"
            className="link-button"
          >
            Downloads and checksums <ArrowUpRight size={15} />
          </a>
        </Section>
        <Section title="Find and install an app">
          <p>
            Search public apps without an account. Replace ring with an app ID
            returned by your search.
          </p>
          <Command value="apps search" />
          <Command value="apps show ring" />
          <Command value="apps install ring" />
          <p>
            The CLI selects a package for your operating system and
            architecture, verifies its checksum and prints the command to run.
            For a package whose command is ring:
          </p>
          <Command value="ring --help" />
          <Command value="apps installed" />
          <Command value="apps daemon status" />
          <p>
            Installation starts automatic updates. To keep the updater running
            after login or restart, register its native startup service.
          </p>
          <Command value="apps daemon install" />
          <Command value="apps uninstall ring" />
          <Command value="apps review ring --rating 5 --text 'Useful, with clear help.'" />
          <p>
            Reviews require <code>apps login</code>. Your account has one review
            per app; saving another changes it.
          </p>
        </Section>
        <Section title="Create your app">
          <p>
            Sign in through Silicon Accounts, then choose your app's permanent
            ID. This guide uses ring; choose your own available ID and replace
            it throughout.
          </p>
          <Command value="apps login" />
          <Command value="apps availability ring" />
          <Command value="apps create ring --name Ring" />
          <p>
            Save the app secret now. It is shown once. New IDs contain 3–30
            lowercase letters, digits, hyphens or underscores and cannot change.
          </p>
          <p>
            Write a 200–600 character introduction in description.txt, then save
            the details. Drafts can be incomplete; publication requires the full
            description and a validated release.
          </p>
          <Command value="apps setup ring details --description-file description.txt --tags tools,productivity" />
          <Command value="apps setup ring access --visibility public" />
          <a href={developerUrl()} className="link-button">
            Open developer platform <ArrowUpRight size={15} />
          </a>
        </Section>
        <Section title="Prepare a package">
          <p>
            Put your executable at package/bin/ring and write package/apps.yaml
            as below. This example targets Apple Silicon macOS; use the actual
            target and native executable you built. Run{" "}
            <code>apps targets</code> for the nine supported target names.
          </p>
          <pre className="docs-code">{`schema_version: 1
app_id: ring
version: 0.1.0
command: ring
targets:
  macos-aarch64:
    binary: bin/ring`}</pre>
          <p>
            Every target executable must support these commands before upload:
          </p>
          <Command value="ring --help" />
          <Command value="ring accounts --json" />
          <Command value="ring login status --json" />
          <p>
            Help exits successfully with useful text. Accounts exits
            successfully with JSON containing <code>{'{"app_id":"ring"}'}</code>
            . In a clean signed-out environment, login status returns{" "}
            <code>{'{"authenticated":false}'}</code>. When signed in, it reports
            true and the Carbon or Silicon identity.
          </p>
          <Command value="apps validate ./package" />
          <Command value="apps pack ./package --output ./ring.tar.gz" />
          <p>
            Validate reports structural errors together. Upload separately
            executes the three commands in an isolated target runner. The
            command output, expected result and failure reason explain what to
            fix. An unavailable worker cannot validate a package.
          </p>
          <Command value="apps docs manifest" />
        </Section>
        <Section title="Upload, release and publish">
          <p>
            Upload one package for every target you support. Copy the package ID
            from the upload result into the release command, then the
            development release ID into the promotion command.
          </p>
          <Command value="apps upload ring --target macos-aarch64 ./ring.tar.gz" />
          <Command value="apps release ring --version 0.1.0 --package PACKAGE_ID" />
          <Command value="apps promote ring DEVELOPMENT_RELEASE_ID --version 1.0.0" />
          <Command value="apps readiness ring" />
          <Command value="apps publish ring" />
          <p>
            Publish makes the app live immediately for its allowed audience. At
            least one validated package in a release is required. Promotion
            supplies a production release for the default install command. A new
            release only needs packages; app details and access carry over.
          </p>
          <Command value="apps docs publish" />
        </Section>
        <Section title="Choose a release and update">
          <p>
            Production and development have independent x.y.z versions. Quote
            references containing &gt; so the shell passes them to Apps.
          </p>
          <Command value="apps install 'ring>dev'" />
          <Command value="apps install 'ring@1.2.3'" />
          <Command value="apps install 'ring>dev@0.1.0'" />
          <Command value="apps update ring" />
          <p>
            The CLI asks before switching channels. An exact version selects the
            initial release; the updater continues following that channel every
            minute. Apps updates itself through the same mechanism. Other apps
            must not run their own updater.
          </p>
          <p>
            Optional install scripts require explicit consent. Use{" "}
            <code>--allow-install-script</code> after reviewing the script;
            consent applies to that app's later updates.
          </p>
        </Section>
        <Section title="Share and maintain your app">
          <Command value="apps authors ring invite c:alice" />
          <Command value="apps authors ring invite si:assistant" />
          <Command value="apps invites list" />
          <Command value="apps invites accept INVITE_ID" />
          <p>
            An invitee becomes an equal author only after accepting. Any author
            can cancel pending invites or leave, except the last remaining
            author. The administrator also controls access, author removal and
            administration transfer.
          </p>
          <Command value="apps setup ring access --visibility private --account c:alice --domain teamofsilicons.com" />
          <p>
            Access changes replace the saved sharing list. Private sharing
            grants discovery and installation, not authorship. Domain access
            requires a verified email. The original creator has no special
            permanent rights.
          </p>
          <Command value="apps history ring" />
          <Command value="apps report 'Describe what happened and what you expected.'" />
        </Section>
        <Section title="Choose where state lives">
          <p>
            The Rust client is stateless. The CLI stores configuration, sessions
            and installs under your chosen home's <code>.apps</code> directory.
            The directory must already exist.
          </p>
          <Command value="apps config home /existing/home" />
          <Command value="apps --home /existing/home installed" />
          <p>
            An explicit <code>--home</code> takes precedence over{" "}
            <code>SILICON_HOME</code>, the saved home and the normal user home.
            Use the same home for sign-in, installation and the updater. Saving
            a home does not migrate files.
          </p>
          <Command value="apps config telemetry off" />
          <p>
            Telemetry is on by default and goes to Space Station when
            configured. Browser settings and CLI settings each control their own
            client.
          </p>
        </Section>
        <Section title="Understand the interfaces">
          <p>
            Every app speaks the same three-command interface so Silicons can
            discover its identity, find instructions and determine who is signed
            in. Accounts owns identity and webhook delivery. Apps owns
            publication, package validation, access, installation and updates.
          </p>
          <p>
            People are stored by immutable Accounts UUID and shown by c:id or
            si:id. Retrying a catalog mutation with the same idempotency key
            avoids performing it twice. Save one-time secrets promptly;
            repeating a request is not a way to recover an expired secret.
          </p>
          <Command value="apps --help" />
          <Command value="apps docs tree" />
          <Command value="apps docs why" />
          <p>
            Each command's <code>--help</code> describes its flags. The bundled
            docs are available without opening a website.
          </p>
        </Section>
      </div>
    </>
  );
}
