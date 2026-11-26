#![forbid(unsafe_code)]
//! CLI precedence tests (`CLI > env > config [server]`) for `--role` and `--salt-store`.

use std::collections::HashMap;

use clap::Parser;
use rotameter_cli::cli::{Cli, Command, GlobalOpts, ServeArgs};
use rotameter_cli::serve_cmd::{SaltStoreSetting, ServeRoleSetting, resolve_serve_settings_with};

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<&str, &str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|v| (*v).to_string())
}

#[test]
fn role_and_salt_store_follow_cli_env_config_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("rotameter.toml");
    std::fs::write(
        &cfg_path,
        r#"
[server]
role = "ingest"
salt_store = "memory"
"#,
    )
    .unwrap();

    let base_global = GlobalOpts {
        config: Some(cfg_path),
        ..GlobalOpts::default()
    };

    let defaults =
        resolve_serve_settings_with(&GlobalOpts::default(), &ServeArgs::default(), env_from(&[]))
            .unwrap();
    assert_eq!(defaults.role, ServeRoleSetting::All);
    assert_eq!(defaults.salt_store, SaltStoreSetting::Db);

    let from_cfg =
        resolve_serve_settings_with(&base_global, &ServeArgs::default(), env_from(&[])).unwrap();
    assert_eq!(from_cfg.role, ServeRoleSetting::Ingest);
    assert_eq!(from_cfg.salt_store, SaltStoreSetting::Memory);

    let from_env = resolve_serve_settings_with(
        &base_global,
        &ServeArgs::default(),
        env_from(&[("ROTAMETER_ROLE", "query"), ("ROTAMETER_SALT_STORE", "db")]),
    )
    .unwrap();
    assert_eq!(from_env.role, ServeRoleSetting::Query);
    assert_eq!(from_env.salt_store, SaltStoreSetting::Db);

    let parsed = Cli::try_parse_from([
        "rotameter",
        "serve",
        "--role",
        "ingest",
        "--salt-store",
        "memory",
    ])
    .unwrap();
    let Command::Serve(serve_args) = parsed.command else {
        panic!("expected serve command");
    };
    let from_cli = resolve_serve_settings_with(
        &base_global,
        &serve_args,
        env_from(&[("ROTAMETER_ROLE", "query"), ("ROTAMETER_SALT_STORE", "db")]),
    )
    .unwrap();
    assert_eq!(from_cli.role, ServeRoleSetting::Ingest);
    assert_eq!(from_cli.salt_store, SaltStoreSetting::Memory);

    let bad_role = resolve_serve_settings_with(
        &GlobalOpts::default(),
        &ServeArgs::default(),
        env_from(&[("ROTAMETER_ROLE", "edge")]),
    );
    assert!(bad_role.is_err());
    let bad_salt = resolve_serve_settings_with(
        &GlobalOpts::default(),
        &ServeArgs::default(),
        env_from(&[("ROTAMETER_SALT_STORE", "redis")]),
    );
    assert!(bad_salt.is_err());
}
