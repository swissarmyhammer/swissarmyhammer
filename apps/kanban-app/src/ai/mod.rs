//! In-process AI agent integration for the kanban app.
//!
//! The ACP agent runs *inside* the kanban-app process. The Tauri Rust backend
//! builds the agent in-process and exposes it on a loopback WebSocket; the
//! webview's TypeScript ACP client connects to that WebSocket. Tauri IPC is
//! not on the ACP data path — the data path is a plain WebSocket.
//!
//! The [`agent_ws::AgentWebSocketServer`] entry point is built here but not
//! yet started from the Tauri setup hook — wiring it into app startup (and
//! handing the bound port to the webview) is a follow-up task. Until then the
//! server type reads as unused in the binary build, hence the module-wide
//! `dead_code` allowance.
#![allow(dead_code)]

pub mod agent_ws;
