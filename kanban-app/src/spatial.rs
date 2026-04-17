//! Tauri commands for spatial focus management.
//!
//! These are transient UI plumbing commands — they manage the spatial entry
//! registry and focused key state. They do NOT flow through `dispatch_command`
//! (no undo/redo, no persistence, no command logging).

use std::collections::HashMap;

use crate::state::AppState;
use serde::Deserialize;
use swissarmyhammer_commands::{BatchEntry, Direction, Rect};
use tauri::{AppHandle, Emitter, State};

/// Register a spatial entry (FocusScope mount or ResizeObserver update).
///
/// Called by React when a FocusScope mounts or its rect changes. The
/// spatial key is a ULID generated client-side, stable across re-renders,
/// unique per mount. The optional `overrides` map allows per-entry
/// navigation redirection or blocking by direction string.
#[tauri::command]
pub async fn spatial_register(
    key: String,
    moniker: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    layer_key: String,
    parent_scope: Option<String>,
    overrides: Option<HashMap<String, Option<String>>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.spatial_state.register(
        key,
        moniker,
        Rect {
            x,
            y,
            width: w,
            height: h,
        },
        layer_key,
        parent_scope,
        overrides.unwrap_or_default(),
    );
    Ok(())
}

/// Unregister a spatial entry (FocusScope unmount).
///
/// If the unregistered entry was the focused key, focus is cleared and a
/// `focus-changed` event is emitted.
#[tauri::command]
pub async fn spatial_unregister(
    key: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.unregister(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Set focus to a spatial key (click or programmatic).
///
/// Updates the focused key and emits a `focus-changed` event if the focus
/// actually changed. No-op if the key is already focused.
#[tauri::command]
pub async fn spatial_focus(
    key: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.focus(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Clear focus without removing any entry.
///
/// Called when React clears focus (e.g. `setFocus(null)`). Emits a
/// `focus-changed` event if something was previously focused.
#[tauri::command]
pub async fn spatial_clear_focus(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(event) = state.spatial_state.clear_focus() {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Navigate from a key in a direction using beam test + scoring.
///
/// Filters to the active layer, applies container-first search, and
/// emits a `focus-changed` event if focus moves. Returns the moniker
/// of the newly focused entry, or `None` if no target was found.
#[tauri::command]
pub async fn spatial_navigate(
    key: String,
    direction: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let dir: Direction = direction
        .parse()
        .map_err(|e: swissarmyhammer_commands::spatial_nav::ParseDirectionError| e.to_string())?;
    match state.spatial_state.navigate(&key, dir)? {
        Some(event) => {
            let next = event.next_key.clone();
            let _ = app.emit("focus-changed", &event);
            Ok(next.and_then(|k| state.spatial_state.get(&k).map(|e| e.moniker)))
        }
        None => Ok(None),
    }
}

/// Push a focus layer onto the layer stack (FocusLayer mount).
///
/// The active (topmost) layer determines which entries are visible to
/// `spatial_navigate`.
#[tauri::command]
pub async fn spatial_push_layer(
    key: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.spatial_state.push_layer(key, name);
    Ok(())
}

/// Remove a focus layer from the layer stack by key (FocusLayer unmount).
///
/// Removal is by key, not pop — supports out-of-order unmount. If the
/// layer below has a `last_focused` key, focus is restored and a
/// `focus-changed` event is emitted.
#[tauri::command]
pub async fn spatial_remove_layer(
    key: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.remove_layer(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Wire format for a single entry in a batch registration call.
///
/// Mirrors the fields of `spatial_register` but packed into a struct so the
/// frontend can send an array in one invoke.
#[derive(Debug, Deserialize)]
pub struct BatchEntryPayload {
    /// Unique spatial key (ULID generated client-side).
    pub key: String,
    /// Entity moniker (e.g. `"task:01ABC"`).
    pub moniker: String,
    /// Left edge x-coordinate.
    pub x: f64,
    /// Top edge y-coordinate.
    pub y: f64,
    /// Width in logical pixels.
    pub w: f64,
    /// Height in logical pixels.
    pub h: f64,
    /// Spatial key of the FocusLayer this scope lives in.
    pub layer_key: String,
    /// Optional parent scope key for container-first navigation.
    pub parent_scope: Option<String>,
    /// Directional navigation overrides.
    pub overrides: Option<HashMap<String, Option<String>>>,
}

/// Register multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer to register estimated rects for off-screen items.
/// Each entry is an upsert — overwrites any existing entry with the same key.
#[tauri::command]
pub async fn spatial_register_batch(
    entries: Vec<BatchEntryPayload>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let batch: Vec<BatchEntry> = entries
        .into_iter()
        .map(|e| BatchEntry {
            key: e.key,
            moniker: e.moniker,
            rect: Rect {
                x: e.x,
                y: e.y,
                width: e.w,
                height: e.h,
            },
            layer_key: e.layer_key,
            parent_scope: e.parent_scope,
            overrides: e.overrides.unwrap_or_default(),
        })
        .collect();
    state.spatial_state.register_batch(batch);
    Ok(())
}

/// Unregister multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer on unmount to clean up placeholder entries.
/// If the focused key was among those removed, a `focus-changed` event
/// is emitted.
#[tauri::command]
pub async fn spatial_unregister_batch(
    keys: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.unregister_batch(&keys) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}
