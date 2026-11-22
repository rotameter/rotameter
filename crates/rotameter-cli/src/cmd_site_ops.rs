#![forbid(unsafe_code)]
//! Helper operations (`delete`, `purge`, and table/JSON/CSV formatting) for `rotameter site`.

#[cfg(feature = "sqlite")]
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rotameter_client::types::{SiteRef, SiteResponse};

use crate::cli::GlobalOpts;
use crate::context::{print_formatted, remote_client};
use crate::exit::CliError;

pub(crate) fn exec_delete(
    global: &GlobalOpts,
    use_local: bool,
    site: &str,
    confirm: bool,
) -> Result<(), CliError> {
    let site_ref = SiteRef::parse(site)?;
    let res = if use_local {
        #[cfg(feature = "sqlite")]
        {
            crate::local_sites::site_delete(global, &site_ref, confirm)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = confirm;
            return Err(CliError::usage(
                "local `--db` mode requires the `sqlite` feature",
            ));
        }
    } else {
        remote_client(global)?.delete_site(&site_ref, confirm)?
    };
    print_formatted(
        global.format,
        &["SITE_ID", "DELETED"],
        &[vec![res.site_id.to_string(), res.deleted.to_string()]],
        &res,
        global.quiet,
    )
}

#[cfg(feature = "sqlite")]
pub(crate) fn run_purge(global: &GlobalOpts, site: &str, before: &str) -> Result<(), CliError> {
    let site_ref = SiteRef::parse(site)?;
    let before_ms = parse_before_ms(before)?;
    let (site_id, deleted) = crate::local_sites::site_purge(global, &site_ref, before_ms)?;
    let payload = serde_json::json!({
        "site_id": site_id,
        "before": before,
        "deleted_rows": deleted,
    });
    print_formatted(
        global.format,
        &["SITE_ID", "DELETED_ROWS"],
        &[vec![site_id.to_string(), deleted.to_string()]],
        &payload,
        global.quiet,
    )
}

#[cfg(not(feature = "sqlite"))]
pub(crate) fn run_purge(_global: &GlobalOpts, _site: &str, _before: &str) -> Result<(), CliError> {
    Err(CliError::usage(
        "`site purge` requires local `--db` and the `sqlite` feature",
    ))
}

#[cfg(feature = "sqlite")]
fn parse_before_ms(raw: &str) -> Result<i64, CliError> {
    let trimmed = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.timestamp_millis());
    }
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        && let Some(naive) = date.and_hms_opt(0, 0, 0)
    {
        return Ok(Utc.from_utc_datetime(&naive).timestamp_millis());
    }
    Err(CliError::usage(format!(
        "invalid `--before` `{raw}`; expected YYYY-MM-DD or RFC 3339"
    )))
}

pub(crate) fn print_sites(global: &GlobalOpts, sites: &[SiteResponse]) -> Result<(), CliError> {
    let rows: Vec<Vec<String>> = sites
        .iter()
        .map(|s| {
            vec![
                s.site_id.to_string(),
                s.slug.clone(),
                s.display_name.clone(),
                s.timezone.clone(),
                s.salt_rotation.as_str().to_string(),
                s.is_self_site.to_string(),
            ]
        })
        .collect();
    print_formatted(
        global.format,
        &[
            "SITE_ID",
            "SLUG",
            "DISPLAY_NAME",
            "TIMEZONE",
            "SALT",
            "SELF",
        ],
        &rows,
        &sites,
        global.quiet,
    )
}
