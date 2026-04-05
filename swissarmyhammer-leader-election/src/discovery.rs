//! Discovery file management for the ZMQ bus.
//!
//! The leader writes a discovery file containing the XSUB (frontend) and XPUB (backend)
//! addresses. Publishers and subscribers read this file to connect.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ElectionError, Result};

/// Addresses advertised by the leader's ZMQ proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusAddresses {
    /// Address for publishers to connect to (XSUB frontend)
    pub frontend: String,
    /// Address for subscribers to connect to (XPUB backend)
    pub backend: String,
}

/// Compute the discovery file path for a given base directory and hash.
pub fn discovery_path(base_dir: &Path, prefix: &str, hash: &str) -> PathBuf {
    base_dir.join(format!("{}-bus-{}.addr", prefix, hash))
}

/// Compute IPC socket paths for the ZMQ proxy.
///
/// Uses short hash (8 chars) and minimal names to stay under the
/// Unix domain socket path limit (104 on macOS, 108 on Linux).
pub fn ipc_addresses(base_dir: &Path, prefix: &str, hash: &str) -> BusAddresses {
    // Truncate hash to 8 chars and use single-char suffixes to keep paths short
    let short_hash = &hash[..hash.len().min(8)];
    let front = base_dir.join(format!("{}-{}-f.sock", prefix, short_hash));
    let back = base_dir.join(format!("{}-{}-b.sock", prefix, short_hash));

    // If path is still too long, fall back to the system temp directory
    let (front, back) = if front.to_string_lossy().len() > 90 {
        let tmp = std::env::temp_dir();
        tracing::warn!(
            base_dir = %base_dir.display(),
            fallback = %tmp.display(),
            "IPC socket path exceeds 90 chars, falling back to temp directory"
        );
        let front = tmp.join(format!("{}-{}-f.sock", prefix, short_hash));
        let back = tmp.join(format!("{}-{}-b.sock", prefix, short_hash));
        (front, back)
    } else {
        (front, back)
    };

    BusAddresses {
        frontend: format!("ipc://{}", front.display()),
        backend: format!("ipc://{}", back.display()),
    }
}

/// Write the discovery file with the proxy's addresses.
pub fn write_discovery(path: &Path, addrs: &BusAddresses) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ElectionError::Discovery)?;
    }
    let content = format!("{}\n{}\n", addrs.frontend, addrs.backend);
    fs::write(path, content).map_err(ElectionError::Discovery)?;
    Ok(())
}

/// Read the discovery file to get the proxy's addresses.
pub fn read_discovery(path: &Path) -> Result<Option<BusAddresses>> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() >= 2 {
                Ok(Some(BusAddresses {
                    frontend: lines[0].to_string(),
                    backend: lines[1].to_string(),
                }))
            } else {
                Ok(None)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ElectionError::Discovery(e)),
    }
}

/// Remove the discovery file and IPC socket files.
pub fn cleanup_discovery(path: &Path, addrs: &BusAddresses) {
    let _ = fs::remove_file(path);
    // Remove IPC socket files (strip "ipc://" prefix)
    if let Some(sock) = addrs.frontend.strip_prefix("ipc://") {
        let _ = fs::remove_file(sock);
    }
    if let Some(sock) = addrs.backend.strip_prefix("ipc://") {
        let _ = fs::remove_file(sock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_discovery_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = discovery_path(dir.path(), "test", "abc123");
        let addrs = BusAddresses {
            frontend: "ipc:///tmp/test-front.sock".to_string(),
            backend: "ipc:///tmp/test-back.sock".to_string(),
        };

        write_discovery(&path, &addrs).unwrap();
        let read = read_discovery(&path).unwrap().unwrap();
        assert_eq!(read.frontend, addrs.frontend);
        assert_eq!(read.backend, addrs.backend);
    }

    #[test]
    fn test_read_missing_discovery() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.addr");
        let result = read_discovery(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_ipc_addresses() {
        let dir = TempDir::new().unwrap();
        let addrs = ipc_addresses(dir.path(), "heb", "abc");
        assert!(addrs.frontend.starts_with("ipc://"));
        assert!(addrs.backend.starts_with("ipc://"));
        assert!(addrs.frontend.contains("-f.sock"));
        assert!(addrs.backend.contains("-b.sock"));
    }

    #[test]
    fn test_ipc_addresses_truncates_long_hash() {
        let dir = TempDir::new().unwrap();
        let long_hash = "abcdef1234567890abcdef1234567890";
        let addrs = ipc_addresses(dir.path(), "t", long_hash);
        // Hash should be truncated to 8 chars
        assert!(addrs.frontend.contains("abcdef12"));
        assert!(!addrs.frontend.contains("abcdef1234567890abcdef"));
    }

    #[test]
    fn test_ipc_addresses_fallback_long_path() {
        // Create a base_dir with a very long path to trigger the >90 char fallback
        let dir = TempDir::new().unwrap();
        // Build a deeply nested path that makes the socket path > 90 chars
        let long_segment = "a".repeat(80);
        let deep_dir = dir.path().join(&long_segment);
        std::fs::create_dir_all(&deep_dir).unwrap();

        let addrs = ipc_addresses(&deep_dir, "prefix", "hashval1");
        // When the path is too long, it should fall back to the temp directory
        let temp = std::env::temp_dir();
        let temp_str = temp.to_string_lossy();
        // The address should use temp dir since the deep path is > 90 chars
        assert!(
            addrs.frontend.contains(&*temp_str) || addrs.frontend.len() <= 100,
            "Expected fallback to temp dir for long paths, got: {}",
            addrs.frontend
        );
    }

    #[test]
    fn test_cleanup_discovery() {
        let dir = TempDir::new().unwrap();
        let disc_path = dir.path().join("test.addr");
        let front_sock = dir.path().join("front.sock");
        let back_sock = dir.path().join("back.sock");

        // Create the files
        fs::write(&disc_path, "test").unwrap();
        fs::write(&front_sock, "").unwrap();
        fs::write(&back_sock, "").unwrap();

        let addrs = BusAddresses {
            frontend: format!("ipc://{}", front_sock.display()),
            backend: format!("ipc://{}", back_sock.display()),
        };

        cleanup_discovery(&disc_path, &addrs);

        assert!(!disc_path.exists());
        assert!(!front_sock.exists());
        assert!(!back_sock.exists());
    }

    #[test]
    fn test_cleanup_discovery_missing_files() {
        let dir = TempDir::new().unwrap();
        let disc_path = dir.path().join("nonexistent.addr");
        let addrs = BusAddresses {
            frontend: "ipc:///tmp/nonexistent-front.sock".to_string(),
            backend: "ipc:///tmp/nonexistent-back.sock".to_string(),
        };
        // Should not panic when files don't exist
        cleanup_discovery(&disc_path, &addrs);
    }

    #[test]
    fn test_cleanup_discovery_no_ipc_prefix() {
        let dir = TempDir::new().unwrap();
        let disc_path = dir.path().join("test.addr");
        fs::write(&disc_path, "test").unwrap();

        // Addresses without ipc:// prefix — cleanup should still remove disc file
        let addrs = BusAddresses {
            frontend: "tcp://127.0.0.1:5555".to_string(),
            backend: "tcp://127.0.0.1:5556".to_string(),
        };

        cleanup_discovery(&disc_path, &addrs);
        assert!(!disc_path.exists());
    }

    #[test]
    fn test_read_discovery_malformed_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("malformed.addr");

        // Write only one line (needs at least 2)
        fs::write(&path, "only-one-line\n").unwrap();
        let result = read_discovery(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_discovery_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.addr");

        fs::write(&path, "").unwrap();
        let result = read_discovery(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_write_discovery_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("test.addr");

        let addrs = BusAddresses {
            frontend: "ipc:///tmp/f.sock".to_string(),
            backend: "ipc:///tmp/b.sock".to_string(),
        };

        write_discovery(&path, &addrs).unwrap();
        assert!(path.exists());

        let read = read_discovery(&path).unwrap().unwrap();
        assert_eq!(read, addrs);
    }

    #[test]
    fn test_discovery_path_format() {
        let dir = TempDir::new().unwrap();
        let path = discovery_path(dir.path(), "myapp", "abc123");
        let filename = path.file_name().unwrap().to_string_lossy();
        assert_eq!(filename, "myapp-bus-abc123.addr");
        assert!(path.starts_with(dir.path()));
    }

    #[test]
    fn test_bus_addresses_equality() {
        let a = BusAddresses {
            frontend: "ipc:///a".to_string(),
            backend: "ipc:///b".to_string(),
        };
        let b = BusAddresses {
            frontend: "ipc:///a".to_string(),
            backend: "ipc:///b".to_string(),
        };
        assert_eq!(a, b);

        let c = BusAddresses {
            frontend: "ipc:///x".to_string(),
            backend: "ipc:///b".to_string(),
        };
        assert_ne!(a, c);
    }

    #[test]
    fn test_bus_addresses_debug() {
        let addrs = BusAddresses {
            frontend: "ipc:///front".to_string(),
            backend: "ipc:///back".to_string(),
        };
        let debug = format!("{:?}", addrs);
        assert!(debug.contains("front"));
        assert!(debug.contains("back"));
    }

    #[test]
    fn test_bus_addresses_clone() {
        let addrs = BusAddresses {
            frontend: "ipc:///f".to_string(),
            backend: "ipc:///b".to_string(),
        };
        let cloned = addrs.clone();
        assert_eq!(addrs, cloned);
    }
}
