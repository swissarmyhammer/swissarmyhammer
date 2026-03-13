//! System tray icon and native menu for the Mirdan desktop app.
//!
//! Builds a macOS/Windows/Linux tray icon with a context menu.
//! "Manage Packages..." opens (or focuses) the webview window.

use tauri::{menu::MenuBuilder, tray::TrayIconBuilder, AppHandle, Manager};

/// Menu item IDs used for event dispatch.
mod ids {
    pub const MANAGE_PACKAGES: &str = "manage-packages";
    pub const CHECK_UPDATES: &str = "check-updates";
    pub const OPEN_REGISTRY: &str = "open-registry";
    pub const QUIT: &str = "quit";
}

/// Show or focus the main webview window.
///
/// If the window exists, show and focus it. The window is defined in
/// tauri.conf.json with `visible: false`, so Tauri creates it at startup
/// but keeps it hidden until we call show().
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        tracing::error!("main window not found");
    }
}

/// Build and attach the system tray icon with its native context menu.
///
/// The menu contains:
/// - A disabled version label
/// - "Manage Packages..." (opens the GUI window)
/// - "Check for Updates" (stub)
/// - "Open Registry" (opens the browser)
/// - "Quit"
///
/// # Errors
///
/// Returns an error if Tauri menu or tray construction fails.
pub fn setup_tray(app: &AppHandle) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    let menu = MenuBuilder::new(app)
        .text("version", format!("Mirdan v{version}"))
        .separator()
        .text(ids::MANAGE_PACKAGES, "Manage Packages...")
        .text(ids::CHECK_UPDATES, "Check for Updates")
        .separator()
        .text(ids::OPEN_REGISTRY, "Open Registry")
        .separator()
        .text(ids::QUIT, "Quit")
        .build()?;

    // Disable the version label so it is not clickable.
    for item in menu.items()? {
        if let tauri::menu::MenuItemKind::MenuItem(mi) = &item {
            if mi.id().as_ref() == "version" {
                mi.set_enabled(false)?;
            }
        }
    }

    // --- Tray icon -----------------------------------------------------------
    // Use the monochrome template icon for the menu bar (adapts to light/dark).
    // Decode the embedded PNG to raw RGBA for Tauri's Image API.
    let tray_png = image::load_from_memory(include_bytes!("../icons/tray-icon@2x.png"))
        .map_err(|e| anyhow::anyhow!("tray icon decode: {e}"))?
        .to_rgba8();
    let (w, h) = tray_png.dimensions();
    let icon = tauri::image::Image::new_owned(tray_png.into_raw(), w, h);

    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .tooltip(format!("Mirdan v{version}"))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            ids::MANAGE_PACKAGES => {
                show_main_window(app);
            }
            ids::CHECK_UPDATES => {
                show_main_window(app);
            }
            ids::OPEN_REGISTRY => {
                if let Err(e) = open::that("https://mirdan.ai") {
                    tracing::error!("failed to open registry URL: {e}");
                }
            }
            ids::QUIT => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
