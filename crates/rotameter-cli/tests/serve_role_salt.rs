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

#[test]
fn duckdb_limits_follow_cli_env_config_precedence_with_vm_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("rotameter.toml");
    std::fs::write(
        &cfg_path,
        r#"
[storage]
duckdb_memory_limit = "384MB"
duckdb_threads = 2
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
    assert_eq!(defaults.duckdb_memory_limit, "256MB");
    assert_eq!(defaults.duckdb_threads, 1);

    let from_cfg =
        resolve_serve_settings_with(&base_global, &ServeArgs::default(), env_from(&[])).unwrap();
    assert_eq!(from_cfg.duckdb_memory_limit, "384MB");
    assert_eq!(from_cfg.duckdb_threads, 2);

    let from_env = resolve_serve_settings_with(
        &base_global,
        &ServeArgs::default(),
        env_from(&[
            ("ROTAMETER_DUCKDB_MEMORY_LIMIT", "512MB"),
            ("ROTAMETER_DUCKDB_THREADS", "4"),
        ]),
    )
    .unwrap();
    assert_eq!(from_env.duckdb_memory_limit, "512MB");
    assert_eq!(from_env.duckdb_threads, 4);

    let parsed = Cli::try_parse_from([
        "rotameter",
        "serve",
        "--duckdb-memory-limit",
        "192MB",
        "--duckdb-threads",
        "3",
    ])
    .unwrap();
    let Command::Serve(serve_args) = parsed.command else {
        panic!("expected serve command");
    };
    let from_cli = resolve_serve_settings_with(
        &base_global,
        &serve_args,
        env_from(&[
            ("ROTAMETER_DUCKDB_MEMORY_LIMIT", "512MB"),
            ("ROTAMETER_DUCKDB_THREADS", "4"),
        ]),
    )
    .unwrap();
    assert_eq!(from_cli.duckdb_memory_limit, "192MB");
    assert_eq!(from_cli.duckdb_threads, 3);

    let zero_env = resolve_serve_settings_with(
        &GlobalOpts::default(),
        &ServeArgs::default(),
        env_from(&[("ROTAMETER_DUCKDB_THREADS", "0")]),
    );
    assert!(zero_env.is_err());

    let parsed_zero = Cli::try_parse_from(["rotameter", "serve", "--duckdb-threads", "0"]).unwrap();
    let Command::Serve(zero_args) = parsed_zero.command else {
        panic!("expected serve command");
    };
    assert!(
        resolve_serve_settings_with(&GlobalOpts::default(), &zero_args, env_from(&[])).is_err()
    );
}

#[test]
fn mcp_http_flags_follow_cli_env_config_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("rotameter.toml");
    std::fs::write(
        &cfg_path,
        "[server]\nenable_mcp_http = true\nmcp_allow_admin = false\n",
    )
    .unwrap();
    let base_global = GlobalOpts {
        config: Some(cfg_path),
        ..GlobalOpts::default()
    };

    let defaults =
        resolve_serve_settings_with(&GlobalOpts::default(), &ServeArgs::default(), env_from(&[]))
            .unwrap();
    assert!(!defaults.mcp_http.enabled);
    assert!(!defaults.mcp_http.allow_admin);

    let from_cfg =
        resolve_serve_settings_with(&base_global, &ServeArgs::default(), env_from(&[])).unwrap();
    assert!(from_cfg.mcp_http.enabled);
    assert!(!from_cfg.mcp_http.allow_admin);

    let from_env = resolve_serve_settings_with(
        &base_global,
        &ServeArgs::default(),
        env_from(&[
            ("ROTAMETER_ENABLE_MCP_HTTP", "false"),
            ("ROTAMETER_MCP_ALLOW_ADMIN", "true"),
        ]),
    )
    .unwrap();
    assert!(!from_env.mcp_http.enabled);
    assert!(from_env.mcp_http.allow_admin);

    let parsed = Cli::try_parse_from([
        "rotameter",
        "serve",
        "--enable-mcp-http",
        "--mcp-allow-admin=false",
    ])
    .unwrap();
    let Command::Serve(serve_args) = parsed.command else {
        panic!("expected serve command");
    };
    let from_cli = resolve_serve_settings_with(
        &base_global,
        &serve_args,
        env_from(&[
            ("ROTAMETER_ENABLE_MCP_HTTP", "false"),
            ("ROTAMETER_MCP_ALLOW_ADMIN", "true"),
        ]),
    )
    .unwrap();
    assert!(from_cli.mcp_http.enabled);
    assert!(!from_cli.mcp_http.allow_admin);
}
