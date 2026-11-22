#![forbid(unsafe_code)]
//! Unit tests for `config.toml` profile resolution, precedence, URL normalization, and redaction.

use std::collections::HashMap;
use std::time::Duration;

use rotameter_client::ClientError;
use rotameter_client::config::{ConfigOverrides, resolve_with};

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<&str, &str> = pairs.iter().copied().collect();
    move |key| map.get(key).map(|v| (*v).to_string())
}

#[test]
fn resolves_precedence_overrides_over_env_over_profile_and_indirection() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        r#"
default_profile = "prod"

[profiles.prod]
url = "https://analytics.example.com/"
site = "colinwolff.dev"
token_env = "PROD_TOKEN"
cf_access_client_id_env = "PROD_CF_ID"
cf_access_client_secret = "inline-secret"

[profiles.staging]
url = "https://staging.example.com/sub/"
site = "staging.dev"
token = "rm_adm_staging_inline"
"#,
    )
    .unwrap();

    // 1. Default profile + *_env indirection + inline fallback.
    let cfg = resolve_with(
        &ConfigOverrides {
            config_path: Some(cfg_path.clone()),
            ..ConfigOverrides::default()
        },
        env_from(&[
            ("PROD_TOKEN", "rm_rd_from_profile_env"),
            ("PROD_CF_ID", "cf-id-from-env"),
        ]),
    )
    .unwrap();
    assert_eq!(cfg.base_url.as_str(), "https://analytics.example.com/");
    assert_eq!(cfg.default_site.as_deref(), Some("colinwolff.dev"));
    assert_eq!(cfg.token.as_deref(), Some("rm_rd_from_profile_env"));
    assert_eq!(cfg.cf_access_client_id.as_deref(), Some("cf-id-from-env"));
    assert_eq!(
        cfg.cf_access_client_secret.as_deref(),
        Some("inline-secret")
    );
    assert_eq!(cfg.timeout, Duration::from_secs(30));

    // 2. Env overrides profile (including ROTAMETER_PROFILE and ROTAMETER_TOKEN).
    let cfg_env = resolve_with(
        &ConfigOverrides {
            config_path: Some(cfg_path.clone()),
            ..ConfigOverrides::default()
        },
        env_from(&[
            ("ROTAMETER_PROFILE", "staging"),
            ("ROTAMETER_TOKEN", "rm_adm_env_override"),
            ("CF_ACCESS_CLIENT_SECRET", "env-cf-secret"),
        ]),
    )
    .unwrap();
    assert_eq!(cfg_env.base_url.as_str(), "https://staging.example.com/sub");
    assert_eq!(cfg_env.default_site.as_deref(), Some("staging.dev"));
    assert_eq!(cfg_env.token.as_deref(), Some("rm_adm_env_override"));
    assert_eq!(
        cfg_env.cf_access_client_secret.as_deref(),
        Some("env-cf-secret")
    );

    // 3. CLI overrides beat env and config.toml.
    let cfg_cli = resolve_with(
        &ConfigOverrides {
            url: Some("http://127.0.0.1:8080/api/".to_string()),
            token: Some("rm_adm_cli_override".to_string()),
            profile: Some("prod".to_string()),
            site: Some("override.dev".to_string()),
            cf_access_client_id: Some("cli-id".to_string()),
            cf_access_client_secret: Some("cli-secret".to_string()),
            config_path: Some(cfg_path),
            timeout: Some(Duration::from_secs(7)),
        },
        env_from(&[
            ("ROTAMETER_URL", "https://ignored.example.com"),
            ("ROTAMETER_TOKEN", "rm_adm_ignored"),
            ("CF_ACCESS_CLIENT_ID", "ignored-id"),
            ("CF_ACCESS_CLIENT_SECRET", "ignored-secret"),
        ]),
    )
    .unwrap();
    assert_eq!(cfg_cli.base_url.as_str(), "http://127.0.0.1:8080/api");
    assert_eq!(cfg_cli.token.as_deref(), Some("rm_adm_cli_override"));
    assert_eq!(cfg_cli.default_site.as_deref(), Some("override.dev"));
    assert_eq!(cfg_cli.cf_access_client_id.as_deref(), Some("cli-id"));
    assert_eq!(
        cfg_cli.cf_access_client_secret.as_deref(),
        Some("cli-secret")
    );
    assert_eq!(cfg_cli.timeout, Duration::from_secs(7));
}

#[test]
fn handles_missing_config_and_unknown_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nonexistent.toml");

    // Missing config.toml succeeds when URL comes from env or overrides.
    let cfg = resolve_with(
        &ConfigOverrides {
            config_path: Some(missing.clone()),
            ..ConfigOverrides::default()
        },
        env_from(&[("ROTAMETER_URL", "http://127.0.0.1:8000")]),
    )
    .unwrap();
    assert_eq!(cfg.base_url.as_str(), "http://127.0.0.1:8000/");

    // Missing URL fails with MissingUrl.
    let err = resolve_with(
        &ConfigOverrides {
            config_path: Some(missing.clone()),
            ..ConfigOverrides::default()
        },
        env_from(&[]),
    )
    .unwrap_err();
    assert!(matches!(err, ClientError::MissingUrl));

    // Explicitly requesting an unknown profile fails with ProfileNotFound even when config is missing.
    let err = resolve_with(
        &ConfigOverrides {
            config_path: Some(missing),
            profile: Some("ghost".to_string()),
            ..ConfigOverrides::default()
        },
        env_from(&[("ROTAMETER_URL", "http://127.0.0.1:8000")]),
    )
    .unwrap_err();
    assert!(matches!(err, ClientError::ProfileNotFound(ref p) if p == "ghost"));

    // XDG_CONFIG_HOME and HOME fallback paths resolve config.toml without explicit config_path.
    let xdg_dir = dir.path().join("xdg");
    std::fs::create_dir_all(xdg_dir.join("rotameter")).unwrap();
    std::fs::write(
        xdg_dir.join("rotameter/config.toml"),
        "default_profile = \"local\"\n[profiles.local]\nurl = \"http://127.0.0.1:8000\"\n",
    )
    .unwrap();
    let from_xdg = resolve_with(
        &ConfigOverrides::default(),
        env_from(&[("XDG_CONFIG_HOME", xdg_dir.to_str().unwrap())]),
    )
    .unwrap();
    assert_eq!(from_xdg.base_url.as_str(), "http://127.0.0.1:8000/");

    let home_dir = dir.path().join("home");
    std::fs::create_dir_all(home_dir.join(".config/rotameter")).unwrap();
    std::fs::write(
        home_dir.join(".config/rotameter/config.toml"),
        "default_profile = \"home\"\n[profiles.home]\nurl = \"https://home.example.com\"\n",
    )
    .unwrap();
    let from_home = resolve_with(
        &ConfigOverrides::default(),
        env_from(&[("HOME", home_dir.to_str().unwrap())]),
    )
    .unwrap();
    assert_eq!(from_home.base_url.as_str(), "https://home.example.com/");
}
