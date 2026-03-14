//! File-lock based leader election primitives for multi-process coordination.
//!
//! This crate provides a reusable leader election mechanism using file locks.
//! The first process to acquire the lock becomes the leader; others become
//! followers that can periodically re-contest the election.
//!
//! # Overview
//!
//! - **File-lock based**: Uses OS-level file locking (`flock`) for reliable election
//! - **Re-election**: Followers can call `try_promote()` to become leader if the lock is free
//! - **Automatic cleanup**: Lock and socket files are cleaned up when the leader exits
//! - **Deterministic paths**: Lock and socket paths are derived from workspace hash
//! - **Configurable**: Prefix and base directory can be customized
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_leader_election::{LeaderElection, ElectionOutcome};
//!
//! let election = LeaderElection::new("/path/to/workspace");
//!
//! match election.elect() {
//!     Ok(ElectionOutcome::Leader(guard)) => {
//!         // We are the leader — guard holds the flock
//!         println!("Leader socket: {:?}", election.socket_path());
//!         // Lock is held as long as `guard` exists
//!     }
//!     Ok(ElectionOutcome::Follower(follower)) => {
//!         // Another process is the leader — we can re-contest later
//!         // follower.try_promote() returns Ok(Some(guard)) if the lock is free
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

mod bus;
pub mod discovery;
mod election;
mod error;
pub mod proxy;

pub use bus::{BusMessage, NullMessage, Publisher, Subscriber};
pub use election::{ElectionConfig, ElectionOutcome, FollowerGuard, LeaderElection, LeaderGuard};
pub use error::{ElectionError, Result};
