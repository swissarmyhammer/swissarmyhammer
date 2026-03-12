//! System tray icon and native menu for the Mirdan desktop app.
//!
//! Builds a macOS/Windows/Linux tray icon with a context menu showing
//! installed packages, an update check placeholder, and a link to the
//! Mirdan registry.

use tauri::{
    menu::MenuBuilder,
    tray::TrayIconBuilder,
    AppHandle,
};

/// Menu item IDs used for event dispatch.
mod ids {
    pub const CHECK_UPDATES: &str = "check-updates";
    pub const OPEN_REGISTRY: &str = "open-registry";
    pub const QUIT: &str = "quit";
}

/// Build and attach the system tray icon with its native context menu.
///
/// The menu contains:
/// - A disabled version label
/// - A submenu listing every installed package (or a placeholder when empty)
/// - "Check for Updates" (stub)
/// - "Open Registry" (opens the browser)
/// - "Quit"
///
/// # Errors
///
/// Returns an error if Tauri menu or tray construction fails.
pub fn setup_tray(app: &AppHandle) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    // --- Installed-packages submenu ------------------------------------------
    let packages = mirdan::list::discover_packages(false, false, false, false, None);

    let mut pkg_submenu = tauri::menu::SubmenuBuilder::new(app, "Installed Packages");

    if packages.is_empty() {
        pkg_submenu = pkg_submenu.text("no-packages", "No packages installed");
    } else {
        for pkg in &packages {
            let label = format!("{} {} ({})", pkg.name, pkg.version, pkg.package_type);
            let id = format!("pkg-{}", pkg.name);
            pkg_submenu = pkg_submenu.text(id, label);
        }
    }

    let pkg_submenu = pkg_submenu.build()?;

    // Disable all items inside the submenu — they are informational only.
    // (Tauri v2 SubmenuBuilder doesn't support per-item enabled on .text(),
    // so we iterate after building.)
    for item in pkg_submenu.items()? {
        if let tauri::menu::MenuItemKind::MenuItem(mi) = item {
            mi.set_enabled(false)?;
        }
    }

    // --- Top-level menu ------------------------------------------------------
    let menu = MenuBuilder::new(app)
        .text("version", format!("Mirdan v{version}"))
        .separator()
        .item(&pkg_submenu)
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
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                ids::CHECK_UPDATES => {
                    eprintln!("[mirdan] check for updates triggered (not yet implemented)");
                }
                ids::OPEN_REGISTRY => {
                    if let Err(e) = open::that("https://registry.mirdan.ai") {
                        eprintln!("[mirdan] failed to open registry URL: {e}");
                    }
                }
                ids::QUIT => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
