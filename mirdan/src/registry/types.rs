//! API request and response types for the Mirdan registry.

use serde::{Deserialize, Serialize};

/// A single result from the fuzzy search endpoint.
#[derive(Debug, Deserialize)]
pub struct FuzzySearchResult {
    pub name: String,
    pub description: String,
    pub author: String,
    #[serde(rename = "type")]
    pub package_type: Option<String>,
    pub downloads: u64,
    pub score: f64,
}

/// Response from `GET /api/search` (fuzzy search).
#[derive(Debug, Deserialize)]
pub struct FuzzySearchResponse {
    pub results: Vec<FuzzySearchResult>,
    pub total: usize,
}

/// Response from `GET /api/packages` (search/list).
#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub packages: Vec<PackageSummary>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

/// Summary of a package in search results.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub name: String,
    pub description: String,
    pub author: String,
    pub latest: String,
    pub tags: Vec<String>,
    #[serde(rename = "type", default)]
    pub package_type: Option<String>,
    pub downloads: u64,
    pub created_at: String,
    pub updated_at: String,
}

/// Detailed package info from `GET /api/packages/:name`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDetail {
    pub name: String,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "type", default)]
    pub package_type: Option<String>,
    pub versions: Vec<String>,
    pub latest: String,
    pub downloads: u64,
    pub created_at: String,
    pub updated_at: String,
    pub readme: Option<String>,
    /// MCP configuration for tool packages.
    #[serde(default)]
    pub mcp: Option<McpConfig>,
    /// Raw TOOL.md content.
    #[serde(default)]
    pub tool_md: Option<String>,
}

/// Version list from `GET /api/packages/:name/versions`.
#[derive(Debug, Deserialize, Serialize)]
pub struct VersionListResponse {
    pub name: String,
    pub versions: Vec<VersionInfo>,
}

/// Version summary in a version list.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub published_at: String,
}

/// MCP server configuration returned by the registry for tool packages.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
}

/// Version detail from `GET /api/packages/:name/latest` or `GET /api/packages/:name/:version`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub name: String,
    pub version: String,
    #[serde(rename = "type", default)]
    pub package_type: Option<String>,
    pub download_url: String,
    #[serde(default)]
    pub integrity: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub published_at: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub tags: Option<Vec<String>>,
    /// MCP configuration for tool packages (when provided by registry).
    #[serde(default)]
    pub mcp: Option<McpConfig>,
    /// Raw TOOL.md content (when provided by registry).
    #[serde(default)]
    pub tool_md: Option<String>,
}

/// Request body for `POST /api/packages/updates`.
#[derive(Debug, Serialize)]
pub struct UpdateCheckRequest {
    pub installed: Vec<InstalledPackage>,
}

/// An installed package entry for update checking.
#[derive(Debug, Serialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
}

/// Response from `POST /api/packages/updates`.
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateCheckResponse {
    pub updates: Vec<PackageUpdate>,
}

/// A single package update entry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageUpdate {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub update_type: String,
}

/// Response from `POST /api/packages` (publish).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResponse {
    pub name: String,
    pub version: String,
    pub published: bool,
    pub download_url: String,
}

/// Response from `DELETE /api/packages/:name/:version` (unpublish).
#[derive(Debug, Deserialize)]
pub struct UnpublishResponse {
    pub deleted: bool,
    pub name: String,
    pub version: String,
}

/// Response from `POST /api/marketplaces` (register marketplace).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceResponse {
    pub id: String,
    pub url: String,
    pub provider: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub registered_by: String,
    pub is_seed: bool,
    pub status: String,
    pub skill_count: u64,
    pub created_at: String,
}

/// A discovered skill from a marketplace sync.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSkill {
    pub qualified_name: String,
    pub name: String,
    pub description: Option<String>,
}

/// Response from `POST /api/marketplaces/:id/sync`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceSyncResponse {
    pub status: String,
    pub skill_count: u64,
    pub discovery_mode: String,
    #[serde(default)]
    pub skills: Vec<DiscoveredSkill>,
}
