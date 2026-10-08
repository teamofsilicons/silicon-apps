# Official Silicon Accounts client

This is an unmodified copy of `crates/client/src` and its README from the official Silicon Accounts repository at commit `00f98860ef42471434f22f019639b8a97830f209`. The package manifest materializes the upstream workspace dependencies so Silicon Apps can build independently. Source: https://github.com/teamofsilicons/silicon-accounts

Version: 0.1.0. License: MIT, as declared in the official package manifest. The corresponding MIT license text is included.

The source includes the Silicon Apps author, catalog export, private mail, webhook preparation and granted verified-email integration from the accompanying Accounts commit. Refresh this directory from that repository rather than making changes to the copy. No separate authentication implementation is maintained here.

The upstream package has not been published to crates.io by this task. Local source builds work through the path dependency; public registry publication requires publishing this official Accounts client first, followed by the Apps package, client and CLI.
