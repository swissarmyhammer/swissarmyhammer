//! Lockfile management for AVP package installations.
//!
//! The lockfile (`avp-lock.json`) tracks installed packages with exact versions
//! and integrity hashes to ensure reproducible installations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

/// Name of the lockfile.
const LOCKFILE_NAME: &str = "avp-lock.json";

/// Current lockfile format version.
const LOCKFILE_VERSION: &str = "1.0.0";

/// Current lockfile schema version.
const LOCKFILE_SCHEMA_VERSION: u32 = 1;

/// Lockfile tracking installed packages with integrity hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: String,
    #[serde(rename = "lockfileVersion")]
    pub lockfile_version: u32,
    pub packages: HashMap<String, LockedPackage>,
}

/// A single locked package entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    pub version: String,
    pub resolved: String,
    pub integrity: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
}

impl Lockfile {
    /// Create a new empty lockfile.
    pub fn new() -> Self {
        Self {
            version: LOCKFILE_VERSION.to_string(),
            lockfile_version: LOCKFILE_SCHEMA_VERSION,
            packages: HashMap::new(),
        }
    }

    /// Load lockfile from the project root directory.
    ///
    /// Returns an empty lockfile if the file doesn't exist.
    pub fn load(project_root: &Path) -> Result<Self, std::io::Error> {
        let path = Self::path(project_root);
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(&path)?;
        serde_json::from_str(&contents).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to parse {}: {}", LOCKFILE_NAME, e),
            )
        })
    }

    /// Save lockfile to the project root directory.
    pub fn save(&self, project_root: &Path) -> Result<(), std::io::Error> {
        let path = Self::path(project_root);
        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to serialize lockfile: {}", e),
            )
        })?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Add or update a package entry.
    pub fn add_package(&mut self, name: String, pkg: LockedPackage) {
        self.packages.insert(name, pkg);
    }

    /// Remove a package entry. Returns the removed entry if it existed.
    pub fn remove_package(&mut self, name: &str) -> Option<LockedPackage> {
        self.packages.remove(name)
    }

    /// Get a package entry by name.
    pub fn get_package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.get(name)
    }

    /// List all packages sorted by name.
    pub fn list_packages(&self) -> Vec<(&String, &LockedPackage)> {
        let mut packages: Vec<_> = self.packages.iter().collect();
        packages.sort_by_key(|(name, _)| name.to_string());
        packages
    }

    /// Get the lockfile path for a project root.
    pub fn path(project_root: &Path) -> PathBuf {
        project_root.join(LOCKFILE_NAME)
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a SHA-512 integrity hash of data in subresource integrity format.
///
/// Returns a string like `"sha512-<base64>"`.
pub fn compute_integrity(data: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(data);
    let result = hasher.finalize();
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, result);
    format!("sha512-{}", encoded)
}

/// Verify that data matches an expected integrity hash.
///
/// Returns `Ok(())` if the hash matches, or an error message if not.
pub fn verify_integrity(data: &[u8], expected: &str) -> Result<(), String> {
    let computed = compute_integrity(data);
    if computed == expected {
        Ok(())
    } else {
        Err(format!(
            "Integrity mismatch: expected {}, got {}",
            expected, computed
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_lockfile() {
        let lf = Lockfile::new();
        assert_eq!(lf.version, "1.0.0");
        assert_eq!(lf.lockfile_version, 1);
        assert!(lf.packages.is_empty());
    }

    #[test]
    fn test_add_and_get_package() {
        let mut lf = Lockfile::new();
        lf.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                resolved: "https://example.com/download".to_string(),
                integrity: "sha512-abc123".to_string(),
                installed_at: "2026-02-06T00:00:00Z".to_string(),
            },
        );

        let pkg = lf.get_package("test-pkg").unwrap();
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.integrity, "sha512-abc123");
    }

    #[test]
    fn test_remove_package() {
        let mut lf = Lockfile::new();
        lf.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                resolved: "https://example.com/download".to_string(),
                integrity: "sha512-abc".to_string(),
                installed_at: "2026-02-06T00:00:00Z".to_string(),
            },
        );

        let removed = lf.remove_package("test-pkg");
        assert!(removed.is_some());
        assert!(lf.get_package("test-pkg").is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut lf = Lockfile::new();
        let removed = lf.remove_package("nope");
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_packages_sorted() {
        let mut lf = Lockfile::new();
        for name in ["zebra", "alpha", "middle"] {
            lf.add_package(
                name.to_string(),
                LockedPackage {
                    version: "1.0.0".to_string(),
                    resolved: String::new(),
                    integrity: String::new(),
                    installed_at: String::new(),
                },
            );
        }
        let list = lf.list_packages();
        let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut lf = Lockfile::new();
        lf.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.2.3".to_string(),
                resolved: "https://registry.example.com/api/packages/test-pkg/1.2.3/download"
                    .to_string(),
                integrity: "sha512-abc123def456".to_string(),
                installed_at: "2026-02-06T10:00:00.000Z".to_string(),
            },
        );

        let json = serde_json::to_string_pretty(&lf).unwrap();
        let parsed: Lockfile = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, lf.version);
        assert_eq!(parsed.lockfile_version, lf.lockfile_version);
        let pkg = parsed.get_package("test-pkg").unwrap();
        assert_eq!(pkg.version, "1.2.3");
        assert_eq!(pkg.integrity, "sha512-abc123def456");
    }

    #[test]
    fn test_json_format_uses_camel_case() {
        let lf = Lockfile::new();
        let json = serde_json::to_string(&lf).unwrap();
        assert!(json.contains("lockfileVersion"));
        assert!(!json.contains("lockfile_version"));
    }

    #[test]
    fn test_locked_package_json_format() {
        let pkg = LockedPackage {
            version: "1.0.0".to_string(),
            resolved: "https://example.com".to_string(),
            integrity: "sha512-test".to_string(),
            installed_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&pkg).unwrap();
        assert!(json.contains("installedAt"));
        assert!(!json.contains("installed_at"));
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let temp = tempfile::tempdir().unwrap();
        let lf = Lockfile::load(temp.path()).unwrap();
        assert!(lf.packages.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let mut lf = Lockfile::new();
        lf.add_package(
            "roundtrip".to_string(),
            LockedPackage {
                version: "2.0.0".to_string(),
                resolved: "https://example.com/dl".to_string(),
                integrity: "sha512-roundtrip".to_string(),
                installed_at: "2026-02-06T12:00:00Z".to_string(),
            },
        );

        lf.save(temp.path()).unwrap();
        let loaded = Lockfile::load(temp.path()).unwrap();
        let pkg = loaded.get_package("roundtrip").unwrap();
        assert_eq!(pkg.version, "2.0.0");
    }

    #[test]
    fn test_compute_integrity() {
        let data = b"hello world";
        let integrity = compute_integrity(data);
        assert!(integrity.starts_with("sha512-"));
        // Same input should produce same hash
        assert_eq!(integrity, compute_integrity(data));
    }

    #[test]
    fn test_compute_integrity_different_data() {
        let a = compute_integrity(b"hello");
        let b = compute_integrity(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn test_verify_integrity_success() {
        let data = b"test data";
        let integrity = compute_integrity(data);
        assert!(verify_integrity(data, &integrity).is_ok());
    }

    #[test]
    fn test_verify_integrity_failure() {
        let data = b"test data";
        let result = verify_integrity(data, "sha512-wronghash");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Integrity mismatch"));
    }
}
