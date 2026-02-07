//! API request and response types for the AVP registry.

use serde::{Deserialize, Serialize};

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
    pub versions: Vec<String>,
    pub latest: String,
    pub downloads: u64,
    pub created_at: String,
    pub updated_at: String,
    pub readme: Option<String>,
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

/// Version detail from `GET /api/packages/:name/latest` or `GET /api/packages/:name/:version`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub name: String,
    pub version: String,
    pub download_url: String,
    pub integrity: String,
    pub size: u64,
    pub published_at: String,
    /// Only present in full version detail, not in latest.
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub tags: Option<Vec<String>>,
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
