#![forbid(unsafe_code)]
//! CLI precedence tests (`CLI > env > config [server]`) for `--ui`, `--ui-dir`, and operator auth.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Parser;
use rotameter_cli::cli::{Cli, Command, GlobalOpts, ServeArgs};
use rotameter_cli::serve_auth_resolve::UiModeSetting;
use rotameter_cli::serve_cmd::resolve_serve_settings_with;

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<&str, &str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|v| (*v).to_string())
}

#[test]
fn ui_and_operator_auth_follow_cli_env_config_precedence() {
    let Ok(dir) = tempfile::tempdir() else {
        std::process::abort()
    };
    let cfg_path = dir.path().join("rotameter.toml");
    assert!(
        std::fs::write(
            &cfg_path,
            r#"
[server]
ui = "on"
ui_dir = "/srv/cfg-ui"
admin_username = "cfgadmin"
admin_password = "cfgpassword"
cf_access_trusted = true

[server.oidc]
issuer_url = "https://idp.example.com"
client_id = "rotameter"
client_secret = "secret"
redirect_uri = "https://analytics.example.com/v1/auth/oidc/callback"
allowed_domains = ["example.com"]
"#,
        )
        .is_ok()
    );
    let base_global = GlobalOpts {
        config: Some(cfg_path),
        ..GlobalOpts::default()
    };

    let Ok(defaults) =
        resolve_serve_settings_with(&GlobalOpts::default(), &ServeArgs::default(), env_from(&[]))
    else {
        std::process::abort()
    };
    assert_eq!(defaults.ui.mode, UiModeSetting::Off);
    assert!(defaults.ui.dir.is_none());
    assert!(!defaults.operator_auth.cf_access_trusted);
    assert!(defaults.operator_auth.oidc.is_none());

    let Ok(from_cfg) =
        resolve_serve_settings_with(&base_global, &ServeArgs::default(), env_from(&[]))
    else {
        std::process::abort()
    };
    assert_eq!(from_cfg.ui.mode, UiModeSetting::On);
    assert_eq!(from_cfg.ui.dir, Some(PathBuf::from("/srv/cfg-ui")));
    assert_eq!(from_cfg.operator_auth.admin_username, "cfgadmin");
    assert!(from_cfg.operator_auth.cf_access_trusted);
    assert_eq!(
        from_cfg
            .operator_auth
            .oidc
            .as_ref()
            .map(|o| o.allowed_domains.as_slice()),
        Some(&["example.com".to_string()][..])
    );

    let Ok(from_env) = resolve_serve_settings_with(
        &base_global,
        &ServeArgs::default(),
        env_from(&[
            ("ROTAMETER_UI", "off"),
            ("ROTAMETER_UI_DIR", "/srv/env-ui"),
            ("ROTAMETER_ADMIN_USERNAME", "envadmin"),
            ("ROTAMETER_CF_ACCESS_TRUSTED", "false"),
        ]),
    ) else {
        std::process::abort()
    };
    assert_eq!(from_env.ui.mode, UiModeSetting::Off);
    assert_eq!(from_env.ui.dir, Some(PathBuf::from("/srv/env-ui")));
    assert_eq!(from_env.operator_auth.admin_username, "envadmin");
    assert!(!from_env.operator_auth.cf_access_trusted);

    let Ok(parsed) = Cli::try_parse_from([
        "rotameter",
        "serve",
        "--ui=on",
        "--ui-dir",
        "/srv/cli-ui",
        "--admin-username",
        "cliadmin",
    ]) else {
        std::process::abort()
    };
    let Command::Serve(serve_args) = parsed.command else {
        std::process::abort()
    };
    let Ok(from_cli) = resolve_serve_settings_with(
        &base_global,
        &serve_args,
        env_from(&[
            ("ROTAMETER_UI", "off"),
            ("ROTAMETER_UI_DIR", "/srv/env-ui"),
            ("ROTAMETER_ADMIN_USERNAME", "envadmin"),
        ]),
    ) else {
        std::process::abort()
    };
    assert_eq!(from_cli.ui.mode, UiModeSetting::On);
    assert_eq!(from_cli.ui.dir, Some(PathBuf::from("/srv/cli-ui")));
    assert_eq!(from_cli.operator_auth.admin_username, "cliadmin");
}
