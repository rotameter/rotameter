#![forbid(unsafe_code)]
//! Synchronous `/v1/admin/*` site and API key control-plane methods.

use crate::blocking::{BlockingRotameterClient, send_json};
use crate::error::ClientError;
use crate::headers::{delete_site_url, endpoint_url};
use crate::types::{
    CreateKeyRequest, CreatedKeyResponse, DeleteSiteResponse, KeyId, KeyMetadataResponse,
    RevokeKeyResponse, SiteRef, SiteResponse, UpsertSiteRequest,
};

impl BlockingRotameterClient {
    /// Creates a new tracked site via `POST /v1/admin/sites` (`rm_adm_` required).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, validation, or slug conflict failure.
    pub fn create_site(&self, req: &UpsertSiteRequest) -> Result<SiteResponse, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "sites"])?;
        send_json(self.http.post(url).json(req))
    }

    /// Idempotently creates or updates `site` via `PUT /v1/admin/sites/{site}` (`rm_adm_` required).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or validation failure.
    pub fn upsert_site(
        &self,
        site: &SiteRef,
        req: &UpsertSiteRequest,
    ) -> Result<SiteResponse, ClientError> {
        let site_seg = site.to_string();
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "sites", &site_seg])?;
        send_json(self.http.put(url).json(req))
    }

    /// Lists all sites in the authenticated org (`GET /v1/admin/sites`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or auth failure.
    pub fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "sites"])?;
        send_json(self.http.get(url))
    }

    /// Fetches configuration for a single site (`GET /v1/admin/sites/{site}`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or not-found failure.
    pub fn get_site(&self, site: &SiteRef) -> Result<SiteResponse, ClientError> {
        let site_seg = site.to_string();
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "sites", &site_seg])?;
        send_json(self.http.get(url))
    }

    /// Deletes a site and cascades its data (`DELETE /v1/admin/sites/{site}?confirm={confirm}`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure, auth failure, or unconfirmed delete.
    pub fn delete_site(
        &self,
        site: &SiteRef,
        confirm: bool,
    ) -> Result<DeleteSiteResponse, ClientError> {
        let url = delete_site_url(&self.config.base_url, site, confirm)?;
        send_json(self.http.delete(url))
    }

    /// Mints a new scoped API key via `POST /v1/admin/keys` (`rm_adm_` required).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or validation failure.
    pub fn create_key(&self, req: &CreateKeyRequest) -> Result<CreatedKeyResponse, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "keys"])?;
        send_json(self.http.post(url).json(req))
    }

    /// Lists non-sensitive API key metadata for the org (`GET /v1/admin/keys`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or auth failure.
    pub fn list_keys(&self) -> Result<Vec<KeyMetadataResponse>, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "keys"])?;
        send_json(self.http.get(url))
    }

    /// Revokes an API key via `DELETE /v1/admin/keys/{key_id}` (`rm_adm_` required).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or not-found failure.
    pub fn revoke_key(&self, key_id: KeyId) -> Result<RevokeKeyResponse, ClientError> {
        let key_seg = key_id.to_string();
        let url = endpoint_url(&self.config.base_url, &["v1", "admin", "keys", &key_seg])?;
        send_json(self.http.delete(url))
    }
}
