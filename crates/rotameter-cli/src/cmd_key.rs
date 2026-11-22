#![forbid(unsafe_code)]
//! `rotameter key` subcommand handler (remote `/v1/admin/keys/*` or local `SQLite`).

use std::str::FromStr;

use rotameter_client::types::{CreateKeyRequest, KeyId, KeyMetadataResponse, KeyScope, SiteRef};

use crate::cli::GlobalOpts;
use crate::cli_sub::{KeyArgs, KeyCommand};
use crate::context::{prefer_local_db, print_formatted, remote_client};
use crate::exit::CliError;

pub(crate) fn run_key(global: &GlobalOpts, args: &KeyArgs) -> Result<(), CliError> {
    let use_local = prefer_local_db(global);
    match &args.action {
        KeyCommand::Create { scope, site, label } => {
            exec_create(global, use_local, scope, site.as_deref(), label)
        }
        KeyCommand::List => exec_list(global, use_local),
        KeyCommand::Revoke { key_id } => exec_revoke(global, use_local, key_id),
    }
}

fn exec_create(
    global: &GlobalOpts,
    use_local: bool,
    scope: &str,
    site: Option<&str>,
    label: &str,
) -> Result<(), CliError> {
    let parsed_scope: KeyScope =
        serde_json::from_value(serde_json::Value::String(scope.trim().to_ascii_lowercase()))
            .map_err(|_| CliError::usage(format!("invalid `--scope` `{scope}`")))?;
    let req = CreateKeyRequest {
        scope: parsed_scope,
        site: site.map(SiteRef::parse).transpose()?,
        label: label.to_string(),
    };
    let res = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_keys::key_create(global, &req)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.create_key(&req)?
    };
    let rows = vec![vec![
        res.key_id.to_string(),
        res.scope.token_prefix().to_string(),
        res.site_id
            .map_or_else(|| "*".to_string(), |id| id.to_string()),
        res.label.clone(),
        res.token.clone(),
    ]];
    print_formatted(
        global.format,
        &["KEY_ID", "PREFIX", "SITE_ID", "LABEL", "TOKEN"],
        &rows,
        &res,
        global.quiet,
    )
}

fn exec_list(global: &GlobalOpts, use_local: bool) -> Result<(), CliError> {
    let keys: Vec<KeyMetadataResponse> = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_keys::key_list(global)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.list_keys()?
    };
    let rows: Vec<Vec<String>> = keys
        .iter()
        .map(|k| {
            vec![
                k.key_id.to_string(),
                k.key_prefix.clone(),
                k.site_id
                    .map_or_else(|| "*".to_string(), |id| id.to_string()),
                k.label.clone(),
                k.revoked_at.clone().unwrap_or_else(|| "active".to_string()),
            ]
        })
        .collect();
    print_formatted(
        global.format,
        &["KEY_ID", "PREFIX", "SITE_ID", "LABEL", "STATUS"],
        &rows,
        &keys,
        global.quiet,
    )
}

fn exec_revoke(global: &GlobalOpts, use_local: bool, key_id: &str) -> Result<(), CliError> {
    let res = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_keys::key_revoke(global, key_id)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = key_id;
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.revoke_key(KeyId::from_str(key_id)?)?
    };
    print_formatted(
        global.format,
        &["KEY_ID", "REVOKED"],
        &[vec![res.key_id.to_string(), res.revoked.to_string()]],
        &res,
        global.quiet,
    )
}
