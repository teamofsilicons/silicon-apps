# Silicon Apps CLI 0.1.8 verification

The command is `silicon-apps`. The installers configure PATH for new terminals and
print the current-shell command. Existing `.apps` state remains in place. Graceful
updater stops survive supervisor restarts, and reinstall waits for an active
update before registering startup again.

Bundled setup scripts run automatically during installation and updates. The
permission flag and installed-record setting are removed, including for old
records containing `false`. Script failure and timeout still restore the previous
package. Windows scripts run relative to the package directory, including nested
paths with spaces. Starter guides use `silicon-apps login --slt TOKEN` for Carbons
and Silicons and explain how Accounts creates the token.

Source `6bf7fde0434815752681665f29eb2f4b5ed77f2c`. All CI jobs passed, including native macOS and Windows
client tests. All nine archives passed executable discovery checks; the 33 public
GitHub assets matched their sizes and SHA-256 digests. Client 0.1.6 and CLI 0.1.8
were published to crates.io, and a fresh registry installation was verified.

The live installer was downloaded and exercised against an isolated home and the
existing Mac installation. Repeated reinstalls, fresh zsh command resolution, and
an explicit stop/start cycle passed. The live store and shared docs passed browser
checks at 390 and 1440 pixels, with no page errors or horizontal overflow. Screenshots
were inspected. Public installers and Markdown matched source.

The Linux catalog archive is `95a3f233da756b601735028fbaf7addc5226787c937439b6a2d9eb194a519471`. Native Linux catalog
installation, command execution, update check and removal of the old setting
passed through SSM `a0e2a2da-eb7a-4292-9ab2-f25f140d30fe`. Evidence SHA-256:
`80bed561180b9ccee0ca43c5d305741dbaefa6275dec303272bbbbf3d6499c3d`. The catalog and validation worker remain
Linux x64 only; Mac self-updates from the catalog remain unavailable. All nine
platforms have independent GitHub downloads.

The API/store deployment remains source `010197db6949a5b11cbed7ca0f3dcc65f4260ef5` with archive
`00cae3ac465a1ff7139a1fe3541cf7fe52e0920f944058baf7b457bb02b7992e`. Installation and postcheck SSM IDs are `f8ee67fe-57a1-4274-bc90-d092d038a55b` and
`4a636b41-56ac-41de-af9d-641e6eede113`. The developer docs deployment is source `56b9c9307988120fad382ec5ef488cb3e2f8fca5`,
archive `ab4f8a9a5cc211da542c4cb253af882328ce8755d351ce07905d6a36e2dee228`. See `cli-release-0.1.8.json` and `production.json`
for release evidence and deployment identities.
