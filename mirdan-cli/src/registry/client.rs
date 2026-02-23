//! HTTP client for the Mirdan registry API.

use bytes::Bytes;
use reqwest::Client;

use crate::auth::{self, Credentials};

use super::error::RegistryError;
use super::types::*;

/// Default registry URL.
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
///
/// Checks MIRDAN_REGISTRY_URL first, then ~/.mirdan/config.yaml, then the default.
pub fn get_registry_url() -> String {
    // Env var takes highest priority
    if let Ok(url) = std::env::var("MIRDAN_REGISTRY_URL") {
        return url;
    }

    // Check config file
    if let Some(home) = dirs::home_dir() {
        let config_path = home.join(".mirdan").join("config.yaml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(url) = yaml.get("registry_url").and_then(|v| v.as_str()) {
                        return url.to_string();
                    }
                }
            }
        }
    }

    DEFAULT_REGISTRY_URL.to_string()
}

/// Client for interacting with the Mirdan registry API.
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

    // -- Unauthenticated endpoints --

    /// Search for packages.
    pub async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<SearchResponse, RegistryError> {
        let mut url = format!(
            "{}/api/packages?q={}",
            self.registry_url,
            urlencoding::encode(query)
        );
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

    /// Fuzzy search for packages (hits `/api/search`).
    pub async fn fuzzy_search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<FuzzySearchResponse, RegistryError> {
        let mut url = format!(
            "{}/api/search?q={}",
            self.registry_url,
            urlencoding::encode(query)
        );
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
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

    // -- Authenticated endpoints --

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

    /// Register a marketplace repository for skill discovery.
    ///
    /// On 409 Conflict (already registered), attempts to parse the existing
    /// marketplace from the response body so callers can still trigger a sync.
    pub async fn register_marketplace(
        &self,
        source: &str,
    ) -> Result<MarketplaceResponse, RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!("{}/api/marketplaces", self.registry_url);

        let body = serde_json::json!({ "source": source });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if status.as_u16() == 409 {
            // Try to parse existing marketplace from conflict response
            let body = response.text().await.unwrap_or_default();
            if let Ok(marketplace) = serde_json::from_str::<MarketplaceResponse>(&body) {
                return Ok(marketplace);
            }
            // If the body contains an embedded marketplace object
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(mp) = json.get("marketplace") {
                    if let Ok(marketplace) =
                        serde_json::from_value::<MarketplaceResponse>(mp.clone())
                    {
                        return Ok(marketplace);
                    }
                }
            }
            return Err(RegistryError::Conflict(extract_error_description(&body)));
        }

        let response = self.check_response(response).await?;
        let result = response.json().await?;
        Ok(result)
    }

    /// Trigger skill discovery sync for a marketplace.
    pub async fn sync_marketplace(
        &self,
        id: &str,
    ) -> Result<MarketplaceSyncResponse, RegistryError> {
        let auth = self.auth_header().ok_or(RegistryError::AuthRequired)?;
        let url = format!("{}/api/marketplaces/{}/sync", self.registry_url, id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth)
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
}
