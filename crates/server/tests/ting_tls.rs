mod common;

#[test]
fn ting_initializes_tls_before_opening_a_secure_websocket() {
    // This integration test runs in its own process so another TLS client cannot
    // hide the ambiguous-provider failure seen only with real wss:// connections.
    assert!(rustls::crypto::CryptoProvider::get_default().is_none());
    let dir = tempfile::tempdir().unwrap();
    let mut config = common::config(dir.path());
    config.accounts_app_secret = Some("test-app-secret".into());
    config.ting_url = Some("https://backend.ting.teamofsilicons.com".into());
    let _publisher = silicon_apps_server::ting::Publisher::configured(&config).unwrap();
    let _tls = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
}
