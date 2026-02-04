//! File-lock based leader election primitives for multi-process coordination.
//!
//! This crate provides a simple, reusable leader election mechanism using file locks.
//! The first process to acquire the lock becomes the leader, and other processes
//! can detect this via a socket file.
//!
//! # Overview
//!
//! - **File-lock based**: Uses OS-level file locking (`flock`) for reliable election
//! - **Deterministic paths**: Lock and socket paths are derived from workspace hash
//! - **Automatic cleanup**: Socket file is cleaned up when leader exits
//! - **Configurable**: Prefix and base directory can be customized
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_leader_election::{LeaderElection, ElectionError};
//!
//! let election = LeaderElection::new("/path/to/workspace");
//!
//! match election.try_become_leader() {
//!     Ok(guard) => {
//!         // We are the leader - guard holds the lock
//!         // Start server on election.socket_path()
//!         println!("Leader socket: {:?}", election.socket_path());
//!
//!         // Lock is held as long as `guard` exists
//!         // When guard is dropped, socket file is cleaned up
//!     }
//!     Err(ElectionError::LockHeld) => {
//!         // Another process is the leader
//!         // Connect to leader at election.socket_path()
//!     }
//!     Err(e) => {
//!         eprintln!("Election failed: {}", e);
//!     }
//! }
//! ```
//!
//! # Custom Configuration
//!
//! ```ignore
//! use swissarmyhammer_leader_election::{LeaderElection, ElectionConfig};
//!
//! let config = ElectionConfig::new()
//!     .with_prefix("myapp")
//!     .with_base_dir("/var/run/myapp");
//!
//! let election = LeaderElection::with_config("/workspace", config);
//! ```

mod election;
mod error;

pub use election::{ElectionConfig, LeaderElection, LeaderGuard};
pub use error::{ElectionError, Result};
