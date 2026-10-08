use silicon_apps_client::{Config, LocalState, updater};
use std::{
    fs,
    io::Read,
    net::TcpListener,
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

// Each probe has a fresh process-global provider. The test reaches an actual TLS
// ClientHello, which the old code replaced with a background-thread panic.
fn probe(name: &str, preserve: bool) {
    if std::env::var_os("APPS_TEST_TELEMETRY_CHILD").is_some() {
        assert!(rustls::crypto::CryptoProvider::get_default().is_none());
        if preserve {
            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .unwrap();
        }
        let before = rustls::crypto::CryptoProvider::get_default().cloned();
        let state = LocalState::new(std::env::var_os("APPS_TEST_TELEMETRY_HOME").unwrap()).unwrap();
        updater::telemetry(
            &state,
            &Config::default(),
            "test",
            "tls",
            1.0,
            serde_json::json!({}),
        );
        let after = rustls::crypto::CryptoProvider::get_default()
            .expect("telemetry must select a provider before spawning its shipper");
        if let Some(before) = before {
            assert!(Arc::ptr_eq(&before, after), "host provider was replaced");
        }
        // Retain recorder diagnostics on Windows, where socket startup failures
        // otherwise disappear behind the intentionally quiet telemetry hook.
        #[cfg(windows)]
        let diagnostic = space_station::SpaceClient::builder(
            &std::env::var("APPS_TELEMETRY_TABLE_KEY").unwrap(),
        )
        .home(state.root.join("telemetry"))
        .url(std::env::var("SPACE_STATION_URL").unwrap())
        .flush_timeout(Duration::from_millis(100))
        .on_error(|error| eprintln!("recorder transport: {error}"))
        .build()
        .unwrap();
        #[cfg(windows)]
        diagnostic.record(serde_json::json!({"event":"tls-test"}));
        let marker = state.home.join("tls-observed");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(marker.exists(), "parent did not observe a TLS handshake");
        return;
    }

    // macOS Unix socket paths have a small fixed length, so keep the telemetry
    // home independent of the long CI checkout and per-user temporary paths.
    #[cfg(unix)]
    let home = tempfile::Builder::new()
        .prefix("apps-tls-")
        .tempdir_in("/tmp")
        .unwrap();
    #[cfg(not(unix))]
    let home = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("APPS_TEST_TELEMETRY_CHILD", "1")
        .env("APPS_TEST_TELEMETRY_HOME", home.path())
        // An inert syntactically valid recording key, sent only to this local socket.
        .env(
            "APPS_TELEMETRY_TABLE_KEY",
            "table-audit-00000000000000000000000000000000",
        )
        .env(
            "SPACE_STATION_URL",
            format!("https://{}", listener.local_addr().unwrap()),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut handshake = None;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut record = [0; 3];
                handshake = Some(socket.read_exact(&mut record).map(|()| record));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("accept loopback TLS: {error}"),
        }
    }
    fs::write(home.path().join("tls-observed"), "done").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("panicked"),
        "background shipper panicked: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = handshake
        .unwrap_or_else(|| {
            panic!(
                "no TLS connection: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        })
        .expect("no TLS record");
    assert_eq!(record[0], 22, "expected TLS handshake record");
    assert_eq!(record[1], 3, "expected TLS protocol version");
}

#[test]
fn telemetry_tls_selects_provider_in_fresh_process() {
    probe("telemetry_tls_selects_provider_in_fresh_process", false);
}

#[test]
fn telemetry_tls_preserves_existing_host_provider() {
    probe("telemetry_tls_preserves_existing_host_provider", true);
}
