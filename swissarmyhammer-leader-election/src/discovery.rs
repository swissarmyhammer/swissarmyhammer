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

    // If path is still too long, use /tmp directly
    let (front, back) = if front.to_string_lossy().len() > 90 {
        let front = PathBuf::from(format!("/tmp/{}-{}-f.sock", prefix, short_hash));
        let back = PathBuf::from(format!("/tmp/{}-{}-b.sock", prefix, short_hash));
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
}
