//! In-process AI agent integration for the kanban app.
//!
//! The ACP agent runs *inside* the kanban-app process. The Tauri Rust backend
//! builds the agent in-process and exposes it on a loopback WebSocket; the
//! webview's TypeScript ACP client connects to that WebSocket. Tauri IPC is
//! not on the ACP data path — the data path is a plain WebSocket.
//!
//! Model selection and the agent-endpoint command surface live in
//! [`models`]: [`models::ai_list_models`] enumerates the selectable models and
//! [`models::ai_start_agent`] prepares an [`agent_ws::AgentWebSocketServer`]
//! for the chosen model, handing the webview its `ws://` and `mcp` URLs.
//!
//! Some accessors here (e.g. [`models::RunningAgent::ws_url`]) are part of the
//! AI backend's public surface but are not yet read by the binary, hence the
//! module-wide `dead_code` allowance.
#![allow(dead_code)]

pub mod agent_ws;
pub mod models;
