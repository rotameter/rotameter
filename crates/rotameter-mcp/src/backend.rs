#![forbid(unsafe_code)]
//! Backend trait and blocking client implementation for `rotameter-mcp`.

use rotameter_client::types::{
    CreateKeyRequest, CreatedKeyResponse, DeleteSiteResponse, KeyId, KeyMetadataResponse,
    QueryRequest, QueryResponse, RealtimeResponse, RevokeKeyResponse, SiteRef, SiteResponse,
    UpsertSiteRequest,
};
use rotameter_client::{BlockingRotameterClient, ClientError};

/// Analytics and optional admin operations exposed to MCP tools.
pub trait AnalyticsBackend {
    /// Lists accessible sites (`GET /v1/admin/sites`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or authorization errors.
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError>;

    /// Executes an analytics query (`POST /v1/query`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or query validation errors.
    fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError>;

    /// Fetches trailing realtime visitor activity (`GET /v1/sites/{site}/realtime`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network, auth, or missing-site errors.
    fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError>;

    /// Returns the optional default site configured on the client profile.
    fn default_site(&self) -> Option<&str>;

    /// Creates or updates a tracked site (`PUT /v1/admin/sites/{site}`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when admin operations are unsupported or fail.
    fn upsert_site(
        &self,
        _site: &SiteRef,
        _req: &UpsertSiteRequest,
    ) -> Result<SiteResponse, ClientError> {
        Err(ClientError::InvalidRequest(
            "admin site upsert is not supported by this backend".to_string(),
        ))
    }

    /// Deletes a tracked site (`DELETE /v1/admin/sites/{site}?confirm={confirm}`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when admin operations are unsupported or fail.
    fn delete_site(
        &self,
        _site: &SiteRef,
        _confirm: bool,
    ) -> Result<DeleteSiteResponse, ClientError> {
        Err(ClientError::InvalidRequest(
            "admin site delete is not supported by this backend".to_string(),
        ))
    }

    /// Mints a new scoped API key (`POST /v1/admin/keys`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when admin operations are unsupported or fail.
    fn create_key(&self, _req: &CreateKeyRequest) -> Result<CreatedKeyResponse, ClientError> {
        Err(ClientError::InvalidRequest(
            "admin key create is not supported by this backend".to_string(),
        ))
    }

    /// Lists non-sensitive API key metadata (`GET /v1/admin/keys`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when admin operations are unsupported or fail.
    fn list_keys(&self) -> Result<Vec<KeyMetadataResponse>, ClientError> {
        Err(ClientError::InvalidRequest(
            "admin key list is not supported by this backend".to_string(),
        ))
    }

    /// Revokes an API key (`DELETE /v1/admin/keys/{key_id}`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when admin operations are unsupported or fail.
    fn revoke_key(&self, _key_id: KeyId) -> Result<RevokeKeyResponse, ClientError> {
        Err(ClientError::InvalidRequest(
            "admin key revoke is not supported by this backend".to_string(),
        ))
    }
}

impl AnalyticsBackend for BlockingRotameterClient {
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError> {
        Self::list_sites(self)
    }

    fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError> {
        Self::query(self, req)
    }

    fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError> {
        Self::realtime(self, site)
    }

    fn default_site(&self) -> Option<&str> {
        self.config().default_site.as_deref()
    }

    fn upsert_site(
        &self,
        site: &SiteRef,
        req: &UpsertSiteRequest,
    ) -> Result<SiteResponse, ClientError> {
        Self::upsert_site(self, site, req)
    }

    fn delete_site(
        &self,
        site: &SiteRef,
        confirm: bool,
    ) -> Result<DeleteSiteResponse, ClientError> {
        Self::delete_site(self, site, confirm)
    }

    fn create_key(&self, req: &CreateKeyRequest) -> Result<CreatedKeyResponse, ClientError> {
        Self::create_key(self, req)
    }

    fn list_keys(&self) -> Result<Vec<KeyMetadataResponse>, ClientError> {
        Self::list_keys(self)
    }

    fn revoke_key(&self, key_id: KeyId) -> Result<RevokeKeyResponse, ClientError> {
        Self::revoke_key(self, key_id)
    }
}
