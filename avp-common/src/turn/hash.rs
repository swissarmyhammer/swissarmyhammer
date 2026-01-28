//! File hashing utilities for change detection.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Hash a file's contents using SHA-256.
///
/// Returns `None` if the file doesn't exist or can't be read.
/// Returns `Some(hash)` with a "sha256:" prefixed hex string if successful.
pub fn hash_file(path: &Path) -> Option<String> {
    match std::fs::read(path) {
        Ok(contents) => {
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let hash = hasher.finalize();
            Some(format!("sha256:{:x}", hash))
        }
        Err(e) => {
            tracing::trace!("Could not hash file '{}': {}", path.display(), e);
            None
        }
    }
}

/// Hash multiple files, returning a map of path to hash.
///
/// Files that don't exist or can't be read will have `None` as their hash.
pub fn hash_files(paths: &[PathBuf]) -> HashMap<PathBuf, Option<String>> {
    paths
        .iter()
        .map(|path| (path.clone(), hash_file(path)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_hash_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "hello world").unwrap();

        let hash = hash_file(&file_path);
        assert!(hash.is_some());
        let hash = hash.unwrap();
        assert!(hash.starts_with("sha256:"));
        // SHA-256 of "hello world" is known
        assert_eq!(
            hash,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hash_file_not_exists() {
        let hash = hash_file(Path::new("/nonexistent/path/to/file.txt"));
        assert!(hash.is_none());
    }

    #[test]
    fn test_hash_file_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();

        let hash = hash_file(&file_path);
        assert!(hash.is_some());
        // SHA-256 of empty string
        assert_eq!(
            hash.unwrap(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_files_mixed() {
        let temp_dir = TempDir::new().unwrap();
        let existing_path = temp_dir.path().join("exists.txt");
        let nonexistent_path = temp_dir.path().join("nonexistent.txt");

        fs::write(&existing_path, "content").unwrap();

        let paths = vec![existing_path.clone(), nonexistent_path.clone()];
        let hashes = hash_files(&paths);

        assert_eq!(hashes.len(), 2);
        assert!(hashes.get(&existing_path).unwrap().is_some());
        assert!(hashes.get(&nonexistent_path).unwrap().is_none());
    }

    #[test]
    fn test_hash_file_binary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("binary.bin");
        fs::write(&file_path, &[0u8, 1, 2, 3, 255, 254, 253]).unwrap();

        let hash = hash_file(&file_path);
        assert!(hash.is_some());
        assert!(hash.unwrap().starts_with("sha256:"));
    }

    #[test]
    fn test_hash_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "same content").unwrap();

        let hash1 = hash_file(&file_path);
        let hash2 = hash_file(&file_path);

        assert_eq!(hash1, hash2);
    }
}
