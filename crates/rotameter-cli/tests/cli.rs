#![forbid(unsafe_code)]
//! CLI tests for `rotameter serve` self-site precedence (`CLI > env > config [server]`).

use std::collections::HashMap;

use clap::Parser;
use rotameter_cli::cli::{Cli, Command, GlobalOpts, ServeArgs};
use rotameter_cli::serve_cmd::resolve_serve_settings_with;

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<&str, &str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|v| (*v).to_string())
}

#[test]
fn serve_self_site_precedence_cli_over_env_over_config() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        r#"
[server]
bind = "127.0.0.1:9001"
db = "cfg.db"
public_url = "https://cfg.example.com"
self_site = false
self_site_id = "cfg-self"
"#,
    )
    .unwrap();

    let global = GlobalOpts {
        config: Some(cfg_path),
        ..GlobalOpts::default()
    };

    // 1. Config [server] applies when CLI and env are unset.
    let from_cfg =
        resolve_serve_settings_with(&global, &ServeArgs::default(), env_from(&[])).unwrap();
    assert!(!from_cfg.self_site_enabled);
    assert_eq!(from_cfg.self_site_id.as_deref(), Some("cfg-self"));
    assert_eq!(
        from_cfg.public_url.as_deref(),
        Some("https://cfg.example.com")
    );
    assert_eq!(from_cfg.db_path, "cfg.db");

    // 2. Env overrides config.
    let from_env = resolve_serve_settings_with(
        &global,
        &ServeArgs::default(),
        env_from(&[
            ("ROTAMETER_SELF_SITE", "true"),
            ("ROTAMETER_SELF_SITE_ID", "env-self"),
            ("ROTAMETER_PUBLIC_URL", "https://env.example.com"),
        ]),
    )
    .unwrap();
    assert!(from_env.self_site_enabled);
    assert_eq!(from_env.self_site_id.as_deref(), Some("env-self"));
    assert_eq!(
        from_env.public_url.as_deref(),
        Some("https://env.example.com")
    );

    // 3. CLI flags (--no-self-site and --self-site=false) override env.
    let parsed = Cli::try_parse_from([
        "rotameter",
        "serve",
        "--self-site=false",
        "--self-site-id",
        "cli-self",
        "--public-url",
        "https://cli.example.com",
    ])
    .unwrap();
    let Command::Serve(serve_args) = parsed.command else {
        panic!("expected serve command");
    };
    let from_cli = resolve_serve_settings_with(
        &global,
        &serve_args,
        env_from(&[
            ("ROTAMETER_SELF_SITE", "true"),
            ("ROTAMETER_SELF_SITE_ID", "env-self"),
            ("ROTAMETER_PUBLIC_URL", "https://env.example.com"),
        ]),
    )
    .unwrap();
    assert!(!from_cli.self_site_enabled);
    assert_eq!(from_cli.self_site_id.as_deref(), Some("cli-self"));
    assert_eq!(
        from_cli.public_url.as_deref(),
        Some("https://cli.example.com")
    );

    let no_ss = Cli::try_parse_from(["rotameter", "serve", "--no-self-site"]).unwrap();
    let Command::Serve(no_ss_args) = no_ss.command else {
        panic!("expected serve command");
    };
    let from_no_ss = resolve_serve_settings_with(
        &global,
        &no_ss_args,
        env_from(&[("ROTAMETER_SELF_SITE", "true")]),
    )
    .unwrap();
    assert!(!from_no_ss.self_site_enabled);
}
