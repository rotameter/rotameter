#![forbid(unsafe_code)]
//! `rotameter site` subcommand handler (remote `/v1/admin/sites/*` or local `SQLite`).

use rotameter_client::types::{SaltRotation, SiteRef, UpsertSiteRequest};

use crate::cli::GlobalOpts;
use crate::cli_sub::{SiteArgs, SiteCommand};
use crate::cmd_site_ops::{exec_delete, print_sites, run_purge};
use crate::context::{prefer_local_db, remote_client};
use crate::exit::CliError;

pub(crate) fn run_site(global: &GlobalOpts, args: &SiteArgs) -> Result<(), CliError> {
    let use_local = prefer_local_db(global);
    match &args.action {
        SiteCommand::Upsert { .. } => exec_upsert(global, use_local, &args.action),
        SiteCommand::List => exec_list(global, use_local),
        SiteCommand::Show { site } => exec_show(global, use_local, site),
        SiteCommand::Purge { site, before } => run_purge(global, site, before),
        SiteCommand::Delete { site, confirm } => exec_delete(global, use_local, site, *confirm),
    }
}

fn exec_upsert(global: &GlobalOpts, use_local: bool, cmd: &SiteCommand) -> Result<(), CliError> {
    let SiteCommand::Upsert {
        site,
        slug,
        name,
        timezone,
        salt_rotation,
        respect_gpc,
        origins,
        allow_query,
        retention_days,
    } = cmd
    else {
        return Ok(());
    };
    let site_ref = SiteRef::parse(site)?;
    let parsed_rot = salt_rotation
        .as_deref()
        .map(parse_salt_rotation)
        .transpose()?;
    let req = UpsertSiteRequest {
        slug: slug.clone(),
        display_name: name.clone(),
        timezone: timezone.clone(),
        salt_rotation: parsed_rot,
        respect_gpc: *respect_gpc,
        allowed_origins: (!origins.is_empty()).then(|| origins.clone()),
        allowed_query_keys: (!allow_query.is_empty()).then(|| allow_query.clone()),
        raw_retention_days: *retention_days,
    };
    let res = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_sites::site_upsert(global, &site_ref, &req)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.upsert_site(&site_ref, &req)?
    };
    print_sites(global, &[res])
}

fn exec_list(global: &GlobalOpts, use_local: bool) -> Result<(), CliError> {
    let sites = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_sites::site_list(global)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.list_sites()?
    };
    print_sites(global, &sites)
}

fn exec_show(global: &GlobalOpts, use_local: bool, site: &str) -> Result<(), CliError> {
    let site_ref = SiteRef::parse(site)?;
    let res = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_sites::site_show(global, &site_ref)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.get_site(&site_ref)?
    };
    print_sites(global, &[res])
}

fn parse_salt_rotation(raw: &str) -> Result<SaltRotation, CliError> {
    serde_json::from_value(serde_json::Value::String(raw.trim().to_ascii_lowercase()))
        .map_err(|_| CliError::usage(format!("invalid `--salt-rotation` `{raw}`")))
}
