/**
 * Frontend logging via tauri-plugin-log.
 *
 * `attachConsole()` is called at app startup (main.tsx) which redirects
 * all console.log/warn/error through Rust's log system automatically.
 *
 * This module re-exports the plugin's explicit log functions for cases
 * where you want to set the level precisely (e.g. `log.debug` won't
 * show unless RUST_LOG includes debug).
 */

export { error, warn, info, debug, trace } from "@tauri-apps/plugin-log";
