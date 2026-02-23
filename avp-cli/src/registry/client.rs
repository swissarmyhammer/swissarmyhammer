//! HTTP client for the AVP registry API.

use bytes::Bytes;
use reqwest::Client;

use crate::auth::{self, Credentials};

use super::error::RegistryError;
use super::types::*;

/// Default registry URL -- the single source of truth.
pub const DEFAULT_REGISTRY_URL: &str = "https://registry.mirdan.ai";

/// Extract a human-readable message from a JSON error body.
///
/// Tries `error_description`, then `message`, then falls back to the raw body.
fn extract_error_description(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(desc) = json.get("error_description").and_then(|v| v.as_str()) {
            return desc.to_string();
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
    }
    body.to_string()
}

/// Get the registry URL from environment or default.
pub fn get_registry_url() -> String {
    std::env::var("AVP_REGISTRY_URL").unwrap_or_else(|_| DEFAULT_REGISTRY_URL.to_string())
}

/// Client for interacting with the AVP registry API.
pub struct RegistryClient {
    client: Client,
    registry_url: String,
    credentials: Option<Credentials>,
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryClient {
    /// Create an unauthenticated client.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            registry_url: get_registry_url(),
            credentials: None,
        }
    }

    /// Create an authenticated client. Fails if no credentials are available.
    pub fn authenticated() -> Result<Self, RegistryError> {
        let credentials = auth::load_credentials().ok_or(RegistryError::AuthRequired)?;
        if credentials.token.is_empty() {
            return Err(RegistryError::AuthRequired);
        }
        Ok(Self {
            client: Client::new(),
            registry_url: get_registry_url(),
            credentials: Some(credentials),
        })
    }

    /// Get the authorization header value, if authenticated.
    fn auth_header(&self) -> Option<String> {
        self.credentials
            .as_ref()
            .map(|c| format!("Bearer {}", c.token))
    }

    /// Map an HTTP response to a `RegistryError` based on status code.
    async fn check_response(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, RegistryError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let status_code = status.as_u16();
        let body = response.text().await.unwrap_or_default();
        let message = extract_error_description(&body);

        match status_code {
            401 => Err(RegistryError::Unauthorized(message)),
            403 => Err(RegistryError::Forbidden(message)),
            404 => Err(RegistryError::NotFound(message)),
            409 => Err(RegistryError::Conflict(message)),
            _ => Err(RegistryError::Api {
                status: status_code,
                body: message,
            }),
        }
    }

    // ── Unauthenticated endpoints ────────────────────────────────────

    /// Search for packages.
    pub async fn search(
        &self,
        query: &str,
        tag: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<SearchResponse, RegistryError> {
        let mut url = format!(
            "{}/api/packages?q={}&type=validator",
            self.registry_url,
            urlencoding::encode(query)
        );
        if let Some(tag) = tag {
            url.push_str(&format!("&tag={}", urlencoding::encode(tag)));
        }
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }
        if let Some(offset) = offset {
            url.push_str(&format!("&offset={}", offset));
        }

        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// Get detailed information about a package.
    pub async fn package_info(&self, name: &str) -> Result<PackageDetail, RegistryError> {
        let url = format!(
            "{}/api/packages/{}",
            self.registry_url,
            urlencoding::encode(name)
        );
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// Get the latest version info for a package.
    pub async fn latest_version(&self, name: &str) -> Result<VersionDetail, RegistryError> {
        let url = format!(
            "{}/api/packages/{}/latest",
            self.registry_url,
            urlencoding::encode(name)
        );
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// Get info about a specific version.
    pub async fn version_info(
        &self,
        name: &str,
        version: &str,
    ) -> Result<VersionDetail, RegistryError> {
        let url = format!(
            "{}/api/packages/{}/{}",
            self.registry_url,
            urlencoding::encode(name),
            urlencoding::encode(version),
        );
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// List all versions for a package.
    pub async fn list_versions(&self, name: &str) -> Result<VersionListResponse, RegistryError> {
        let url = format!(
            "{}/api/packages/{}/versions",
            self.registry_url,
            urlencoding::encode(name)
        );
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    // ── Authenticated endpoints ──────────────────────────────────────

    /// Download a package version as bytes.
    pub async fn download(&self, name: &str, version: &str) -> Result<Bytes, RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!(
            "{}/api/packages/{}/{}/download",
            self.registry_url,
            urlencoding::encode(name),
            urlencoding::encode(version),
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        let bytes = response.bytes().await?;
        Ok(bytes)
    }

    /// Publish a package (ZIP archive).
    pub async fn publish(&self, archive: Vec<u8>) -> Result<PublishResponse, RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!("{}/api/packages", self.registry_url);

        let part = reqwest::multipart::Part::bytes(archive)
            .file_name("package.zip")
            .mime_str("application/zip")
            .map_err(|e| RegistryError::Validation(e.to_string()))?;
        let form = reqwest::multipart::Form::new().part("package", part);

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth)
            .multipart(form)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// Unpublish (delete) a specific version.
    pub async fn unpublish(&self, name: &str, version: &str) -> Result<(), RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!(
            "{}/api/packages/{}/{}",
            self.registry_url,
            urlencoding::encode(name),
            urlencoding::encode(version),
        );
        let response = self
            .client
            .delete(&url)
            .header("Authorization", &auth)
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Check for updates to installed packages.
    pub async fn check_updates(
        &self,
        installed: Vec<InstalledPackage>,
    ) -> Result<UpdateCheckResponse, RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!("{}/api/packages/updates", self.registry_url);
        let body = UpdateCheckRequest { installed };

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth)
            .json(&body)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }
}
