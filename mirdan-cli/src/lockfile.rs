//! Mirdan lockfile management (mirdan-lock.json).
//!
//! Tracks installed packages with version, integrity, type, and deployment targets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::package_type::PackageType;
use crate::registry::RegistryError;

/// Lockfile filename.
const LOCKFILE_NAME: &str = "mirdan-lock.json";

/// A locked package entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    /// Package type (skill or validator).
    #[serde(rename = "type")]
    pub package_type: PackageType,
    /// Installed version.
    pub version: String,
    /// Download URL from registry.
    pub resolved: String,
    /// SHA-512 integrity hash.
    pub integrity: String,
    /// ISO-8601 timestamp of installation.
    pub installed_at: String,
    /// Target agents (for skills) or paths (for validators).
    pub targets: Vec<String>,
}

/// The mirdan-lock.json file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version.
    #[serde(default = "default_lockfile_version")]
    pub lockfile_version: u32,
    /// Map of package name -> locked entry.
    pub packages: BTreeMap<String, LockedPackage>,
}

fn default_lockfile_version() -> u32 {
    1
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            lockfile_version: 1,
            packages: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    /// Load lockfile from a project root directory.
    ///
    /// Returns an empty lockfile if the file doesn't exist.
    pub fn load(project_root: &Path) -> Result<Self, RegistryError> {
        let path = lockfile_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(|e| {
            RegistryError::Validation(format!("Invalid lockfile '{}': {}", path.display(), e))
        })
    }

    /// Save lockfile to a project root directory.
    pub fn save(&self, project_root: &Path) -> Result<(), RegistryError> {
        let path = lockfile_path(project_root);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add or replace a package entry.
    pub fn add_package(&mut self, name: String, entry: LockedPackage) {
        self.packages.insert(name, entry);
    }

    /// Remove a package entry.
    pub fn remove_package(&mut self, name: &str) -> Option<LockedPackage> {
        self.packages.remove(name)
    }

    /// Get a package entry.
    pub fn get_package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.get(name)
    }
}

/// Get the lockfile path for a project root.
fn lockfile_path(project_root: &Path) -> PathBuf {
    project_root.join(LOCKFILE_NAME)
}

/// Verify SHA-512 integrity of downloaded data.
pub fn verify_integrity(data: &[u8], expected: &str) -> Result<(), String> {
    use base64::Engine;
    use sha2::Digest;

    // Expected format: "sha512-<base64>"
    let hash_b64 = expected
        .strip_prefix("sha512-")
        .ok_or_else(|| format!("Expected sha512- prefix, got: {}", expected))?;

    let mut hasher = sha2::Sha512::new();
    hasher.update(data);
    let actual = base64::engine::general_purpose::STANDARD.encode(hasher.finalize());

    if actual != hash_b64 {
        return Err(format!(
            "Integrity mismatch: expected {}, got sha512-{}",
            expected, actual
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfile_default() {
        let lf = Lockfile::default();
        assert_eq!(lf.lockfile_version, 1);
        assert!(lf.packages.is_empty());
    }

    #[test]
    fn test_lockfile_roundtrip() {
        let dir = tempfile::tempdir().unwrap();

        let mut lf = Lockfile::default();
        lf.add_package(
            "test-skill".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "https://example.com/test-skill-1.0.0.zip".to_string(),
                integrity: "sha512-abc123".to_string(),
                installed_at: "2024-01-01T00:00:00Z".to_string(),
                targets: vec!["claude-code".to_string(), "cursor".to_string()],
            },
        );

        lf.save(dir.path()).unwrap();
        let loaded = Lockfile::load(dir.path()).unwrap();

        assert_eq!(loaded.packages.len(), 1);
        let pkg = loaded.get_package("test-skill").unwrap();
        assert_eq!(pkg.package_type, PackageType::Skill);
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.targets.len(), 2);
    }

    #[test]
    fn test_lockfile_load_missing() {
        let dir = tempfile::tempdir().unwrap();
        let lf = Lockfile::load(dir.path()).unwrap();
        assert!(lf.packages.is_empty());
    }

    #[test]
    fn test_lockfile_remove_package() {
        let mut lf = Lockfile::default();
        lf.add_package(
            "test".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "0.1.0".to_string(),
                resolved: String::new(),
                integrity: String::new(),
                installed_at: String::new(),
                targets: vec![],
            },
        );
        assert!(lf.get_package("test").is_some());
        lf.remove_package("test");
        assert!(lf.get_package("test").is_none());
    }

    #[test]
    fn test_verify_integrity_valid() {
        use base64::Engine;
        use sha2::Digest;

        let data = b"hello world";
        let mut hasher = sha2::Sha512::new();
        hasher.update(data);
        let hash = base64::engine::general_purpose::STANDARD.encode(hasher.finalize());
        let integrity = format!("sha512-{}", hash);

        assert!(verify_integrity(data, &integrity).is_ok());
    }

    #[test]
    fn test_verify_integrity_invalid() {
        assert!(verify_integrity(b"hello", "sha512-invalid").is_err());
    }

    #[test]
    fn test_verify_integrity_bad_prefix() {
        assert!(verify_integrity(b"hello", "md5-abc").is_err());
    }
}
