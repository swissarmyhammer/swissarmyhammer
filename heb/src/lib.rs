//! HEB — Hook Event Bus
//!
//! Real-time event pub/sub with SQLite durability, built on top of
//! `swissarmyhammer-leader-election`'s typed bus.
//!
//! # Overview
//!
//! Every participant in HEB (hooks, agents, UIs) can publish and subscribe
//! to events. The bus is embedded in the leader election process — whoever
//! wins the election runs the ZMQ XPUB/XSUB proxy. Everyone else connects.
//!
//! Events are persisted to SQLite (open/write/close per event) independently
//! by each publisher. ZMQ is the live delivery path. SQLite is the durable log.
//!
//! # Example
//!
//! ```ignore
//! use heb::{HebContext, EventHeader, EventCategory};
//!
//! let ctx = HebContext::open(workspace_root)?;
//!
//! let header = EventHeader::new(
//!     session_id, cwd,
//!     EventCategory::Hook,
//!     "pre_tool_use",
//!     "avp-hook",
//! );
//! let id = ctx.publish(&header, body_bytes)?;
//!
//! // Replay missed events after reconnect
//! let events = ctx.replay(&last_seen_id, Some("hook"))?;
//! ```

mod context;
pub mod error;
mod event;
pub mod header;
pub mod store;

pub use context::HebContext;
pub use error::{HebError, Result};
pub use event::HebEvent;
pub use header::{EventCategory, EventHeader};
