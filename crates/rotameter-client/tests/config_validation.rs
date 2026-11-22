#![forbid(unsafe_code)]
//! Unit tests for URL validation and secret redaction in `Debug` output.

use rotameter_client::config::ClientConfig;
use rotameter_client::{BlockingRotameterClient, ClientError, RotameterClient};

#[test]
fn validates_urls_and_redacts_secrets_in_debug() {
    assert!(matches!(
        ClientConfig::new("not a url"),
        Err(ClientError::InvalidUrl(_))
    ));
    assert!(matches!(
        ClientConfig::new("ftp://analytics.example.com"),
        Err(ClientError::InvalidUrl(_))
    ));

    let cfg = ClientConfig::new("https://analytics.example.com/sub/?ignored=1#frag")
        .unwrap()
        .with_token("rm_adm_super_secret_token")
        .with_cf_access("public-client-id", "super-secret-cf-value");
    assert_eq!(cfg.base_url.as_str(), "https://analytics.example.com/sub");

    let cfg_debug = format!("{cfg:?}");
    assert!(!cfg_debug.contains("rm_adm_super_secret_token"));
    assert!(!cfg_debug.contains("super-secret-cf-value"));
    assert!(cfg_debug.contains("[REDACTED]"));
    assert!(cfg_debug.contains("public-client-id"));

    let async_client = RotameterClient::new(cfg.clone()).unwrap();
    let async_debug = format!("{async_client:?}");
    assert!(!async_debug.contains("rm_adm_super_secret_token"));
    assert!(!async_debug.contains("super-secret-cf-value"));

    let blocking_client = BlockingRotameterClient::new(cfg).unwrap();
    let blocking_debug = format!("{blocking_client:?}");
    assert!(!blocking_debug.contains("rm_adm_super_secret_token"));
    assert!(!blocking_debug.contains("super-secret-cf-value"));
}
