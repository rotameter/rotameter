#![forbid(unsafe_code)]
//! `GeoIP` resolution combining trusted proxy headers and optional `MaxMind` `.mmdb` fallback.

use std::net::IpAddr;

use axum::http::HeaderMap;
use maxminddb::{Reader, geoip2};

use crate::config::{GeoIpMode, ServerConfig, ServerError};
use crate::handlers::headers::{extract_country, extract_region, normalize_country};

pub(crate) struct GeoIpResolver {
    trusted_proxy_headers: bool,
    mmdb: Option<Reader<Vec<u8>>>,
}

impl GeoIpResolver {
    pub(crate) fn from_config(config: &ServerConfig) -> Result<Self, ServerError> {
        let mmdb = match config.geoip_mode {
            GeoIpMode::Header => None,
            GeoIpMode::Mmdb => {
                let path = config.mmdb_path.as_deref().ok_or_else(|| {
                    ServerError::new("geoip_mode=mmdb requires `mmdb_path` to be configured")
                })?;
                let reader = Reader::open_readfile(path).map_err(|err| {
                    ServerError::new(format!(
                        "failed to open MMDB database `{}`: {err}",
                        path.display()
                    ))
                })?;
                Some(reader)
            }
        };
        Ok(Self {
            trusted_proxy_headers: config.trusted_proxy_headers,
            mmdb,
        })
    }

    pub(crate) fn resolve(
        &self,
        headers: &HeaderMap,
        client_ip: &str,
        authenticated_proxy: bool,
    ) -> (String, String) {
        let trust_cf = self.trusted_proxy_headers || authenticated_proxy;
        let header_country = if trust_cf {
            Some(extract_country(headers, authenticated_proxy)).filter(|c| c != "(unknown)")
        } else {
            None
        };
        let header_region = if trust_cf {
            Some(extract_region(headers, authenticated_proxy)).filter(|r| r != "(unknown)")
        } else {
            None
        };

        if let Some(country) = header_country {
            return (
                country,
                header_region.unwrap_or_else(|| "(unknown)".to_string()),
            );
        }

        if let Some(reader) = self.mmdb.as_ref()
            && let Some((country, region)) = lookup_mmdb(reader, client_ip)
        {
            return (country, header_region.unwrap_or(region));
        }

        (
            "(unknown)".to_string(),
            header_region.unwrap_or_else(|| "(unknown)".to_string()),
        )
    }
}

fn lookup_mmdb(reader: &Reader<Vec<u8>>, client_ip: &str) -> Option<(String, String)> {
    let addr: IpAddr = client_ip.trim().parse().ok()?;
    let city: geoip2::City<'_> = reader.lookup(addr).ok()?.decode().ok()??;
    let country_raw = city.country.iso_code?;
    let country = normalize_country(Some(country_raw.to_string()))?;
    let region = city
        .subdivisions
        .first()
        .and_then(|sub| sub.iso_code)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(|| "(unknown)".to_string(), ToString::to_string);
    Some((country, region))
}
