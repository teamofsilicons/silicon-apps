# A working sample package

`hello-apps` is a POSIX shell app with real implementations of all three required discovery commands. It has no sign-in or updater of its own and is always signed out. It needs `/bin/sh`, so the sample supports Unix targets only; the package format and Apps CLI also support three native Windows targets.

From the repository root:

```sh
cargo run -p silicon-apps-cli -- validate sample-package
cargo run -p silicon-apps-cli -- pack sample-package --output /tmp/hello-apps.tar.gz
sample-package/bin/hello-apps --help
sample-package/bin/hello-apps accounts --json
sample-package/bin/hello-apps login status --json
```

Create the `hello-apps` app before uploading. Upload only targets whose isolated runners are configured. A local structural validation does not imply that every target's isolated runtime validation has passed.
