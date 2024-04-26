use nyar_package_registry::types::RetryConfig;

#[test]
fn npm_upload_timeout_is_generous() {
    let config = RetryConfig::npm();
    assert_eq!(config.max_retries, 5);
    assert_eq!(config.upload_timeout_secs, 600);
    assert_eq!(config.connect_timeout_secs, 30);
}
