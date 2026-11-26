#![forbid(unsafe_code)]
//! Storage DSN parser for `--db` / `--control-db` and `--analytics-db`.

use std::path::PathBuf;

use crate::exit::CliError;

/// Parsed storage connection target for Control Plane or Analytics Data Plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageDsn {
    /// Local `SQLite` WAL file path (or `:memory:`).
    Sqlite(PathBuf),
    /// Local `DuckDB` analytical database file path (requires `--features duckdb`).
    DuckDb(PathBuf),
}

impl StorageDsn {
    /// Parses a bare filesystem path, `:memory:`, `sqlite://...`, or `duckdb://...` DSN.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::usage`] if the DSN is empty or uses an unsupported scheme.
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CliError::usage("storage DSN cannot be empty"));
        }
        if let Some(rest) = trimmed.strip_prefix("sqlite://") {
            let path = strip_scheme_path(rest, trimmed)?;
            return Ok(Self::Sqlite(path));
        }
        if let Some(rest) = trimmed.strip_prefix("sqlite:") {
            let path = strip_scheme_path(rest, trimmed)?;
            return Ok(Self::Sqlite(path));
        }
        if let Some(rest) = trimmed.strip_prefix("duckdb://") {
            let path = strip_scheme_path(rest, trimmed)?;
            return Ok(Self::DuckDb(path));
        }
        if let Some(rest) = trimmed.strip_prefix("duckdb:") {
            let path = strip_scheme_path(rest, trimmed)?;
            return Ok(Self::DuckDb(path));
        }
        if let Some((scheme, _)) = trimmed.split_once("://") {
            return Err(CliError::usage(format!(
                "unsupported storage DSN scheme `{scheme}://` (expected bare path, `sqlite://`, or `duckdb://`)"
            )));
        }
        Ok(Self::Sqlite(PathBuf::from(trimmed)))
    }
}

fn strip_scheme_path(rest: &str, original: &str) -> Result<PathBuf, CliError> {
    let cleaned = rest.trim();
    if cleaned.is_empty() {
        return Err(CliError::usage(format!(
            "storage DSN `{original}` is missing a database path"
        )));
    }
    Ok(PathBuf::from(cleaned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_sqlite_and_duckdb_dsns() {
        assert_eq!(
            StorageDsn::parse("rotameter.db").unwrap(),
            StorageDsn::Sqlite(PathBuf::from("rotameter.db"))
        );
        assert_eq!(
            StorageDsn::parse(":memory:").unwrap(),
            StorageDsn::Sqlite(PathBuf::from(":memory:"))
        );
        assert_eq!(
            StorageDsn::parse("sqlite:///var/lib/rotameter/rotameter.db").unwrap(),
            StorageDsn::Sqlite(PathBuf::from("/var/lib/rotameter/rotameter.db"))
        );
        assert_eq!(
            StorageDsn::parse("sqlite://data/control.db").unwrap(),
            StorageDsn::Sqlite(PathBuf::from("data/control.db"))
        );
        assert_eq!(
            StorageDsn::parse("duckdb:///data/analytics.duckdb").unwrap(),
            StorageDsn::DuckDb(PathBuf::from("/data/analytics.duckdb"))
        );
        assert_eq!(
            StorageDsn::parse("duckdb:analytics.duckdb").unwrap(),
            StorageDsn::DuckDb(PathBuf::from("analytics.duckdb"))
        );
    }

    #[test]
    fn rejects_empty_missing_path_and_unknown_schemes() {
        assert!(StorageDsn::parse("   ").is_err());
        assert!(StorageDsn::parse("sqlite://").is_err());
        assert!(StorageDsn::parse("duckdb://").is_err());
        assert!(StorageDsn::parse("postgres://localhost/rotameter").is_err());
    }
}
