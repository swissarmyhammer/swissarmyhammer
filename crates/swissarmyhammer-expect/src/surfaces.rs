//! The surface adapter catalog: a static, no-IO description of each built-in
//! adapter from `ideas/expect.md` §"Surface adapters".
//!
//! Each [`SurfaceInfo`] records how an adapter **drives** (causes the When),
//! **observes** (captures the authoritative checkpoint), its in-process
//! **mechanism**, and its per-surface **locator dialect**. This is pure
//! documentation-of-intent data — no IO, no system access, no agent — so the
//! `surface get` / `surfaces list` read ops can serve it directly.
//!
//! Every adapter is `deterministic`: actuation is mechanical (run argv, issue a
//! request, press `role[name=…]`), so a run is reproducible and may run once.
//! The *only* source of non-determinism is the runtime **agent fallback** (an
//! agent driving the mechanical loop), which is the exception — never a property
//! of the surface itself.

use serde::{Deserialize, Serialize};

use crate::types::Surface;

/// Every built-in adapter is mechanically actuated, hence reproducible. The only
/// non-determinism source is the runtime agent fallback, not the surface (see
/// the module docs), so every catalog entry is `deterministic: true`.
const DETERMINISTIC: bool = true;

/// One surface adapter's catalog entry: how it drives, observes, its in-process
/// mechanism, and its locator dialect.
///
/// Sourced from the design tables in `ideas/expect.md` — drive/observe/mechanism
/// from §"Surface adapters", `locator_dialect` from §"Locators are a per-surface
/// dialect". Pure data: it round-trips through `serde_json` like the rest of the
/// domain model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceInfo {
    /// Which adapter this describes.
    pub name: Surface,
    /// How the adapter causes the `When` transition.
    pub drive: String,
    /// What the adapter captures as the authoritative checkpoint state.
    pub observe: String,
    /// The in-process Rust mechanism (no Node/Python/Playwright/Appium).
    pub mechanism: String,
    /// The per-surface locator dialect used to resolve a path into a checkpoint's
    /// state.
    pub locator_dialect: String,
    /// Whether actuation is mechanical and reproducible. Always `true` — the only
    /// non-determinism source is the runtime agent fallback, never the surface.
    pub deterministic: bool,
}

/// The full surface adapter catalog, one [`SurfaceInfo`] per built-in adapter.
///
/// The single source of truth for `surfaces list` and `surface get`, built from
/// the design tables in `ideas/expect.md`. The columns are
/// `(name, drive, observe, mechanism, locator_dialect)`; every entry is
/// `deterministic` (see [`DETERMINISTIC`]).
pub fn catalog() -> Vec<SurfaceInfo> {
    const ROWS: &[(Surface, &str, &str, &str, &str)] = &[
        (
            Surface::Cli,
            "run argv",
            "stdout/stderr/exit/files",
            "std process",
            "stream regex-capture / json-path if JSON / `exit`",
        ),
        (
            Surface::Http,
            "issue request",
            "status/headers/body",
            "an HTTP client",
            "`status` / `header:<name>` / json-path",
        ),
        (
            Surface::Browser,
            "press/type by `role[name=…]`",
            "snapshot the a11y tree",
            "CDP `Accessibility` + `Input` via `chromiumoxide` (pure Rust, no Node)",
            "`role[name=…]` + tree relationship (`within` / `ancestor`)",
        ),
        (
            Surface::Gui,
            "press/type by `role[name=…]`",
            "snapshot the a11y tree",
            "AX (macOS `AXUIElement`) · UIA (Windows `IUIAutomation`) · AT-SPI (Linux `atspi`+`zbus`)",
            "`role[name=…]` + tree relationship (`within` / `ancestor`)",
        ),
        (
            Surface::File,
            "write",
            "files/dirs/content",
            "the filesystem",
            "path + content (+ sub-locator if structured)",
        ),
        (
            Surface::Db,
            "run statements",
            "rows/tables",
            "a DB client",
            "a SQL query + projection",
        ),
    ];

    ROWS.iter()
        .map(
            |&(name, drive, observe, mechanism, locator_dialect)| SurfaceInfo {
                name,
                drive: drive.to_string(),
                observe: observe.to_string(),
                mechanism: mechanism.to_string(),
                locator_dialect: locator_dialect.to_string(),
                deterministic: DETERMINISTIC,
            },
        )
        .collect()
}

/// Look up one surface adapter's catalog entry by name.
///
/// Returns `None` only if `name` is somehow absent from the catalog; since
/// [`Surface`] is a closed enum every catalog covers, a successful parse of a
/// surface name always resolves here.
pub fn get(name: Surface) -> Option<SurfaceInfo> {
    catalog().into_iter().find(|info| info.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closed set of surface adapters the catalog must describe, in
    /// `Surface` enum declaration order. The coverage/count tests check the
    /// catalog against this set rather than re-typing the names or count.
    const ALL_SURFACES: &[Surface] = &[
        Surface::Cli,
        Surface::Http,
        Surface::Browser,
        Surface::Gui,
        Surface::File,
        Surface::Db,
    ];

    #[test]
    fn catalog_has_one_entry_per_surface() {
        assert_eq!(catalog().len(), ALL_SURFACES.len());
    }

    #[test]
    fn catalog_covers_every_surface_exactly_once() {
        let names: Vec<Surface> = catalog().into_iter().map(|info| info.name).collect();
        for surface in ALL_SURFACES {
            let hits = names.iter().filter(|&&n| n == *surface).count();
            assert_eq!(
                hits, 1,
                "{surface:?} must appear exactly once in the catalog"
            );
        }
    }

    #[test]
    fn every_surface_is_deterministic() {
        assert!(
            catalog().iter().all(|info| info.deterministic),
            "every surface adapter is mechanically actuated, hence deterministic"
        );
    }

    #[test]
    fn every_entry_has_populated_fields() {
        for info in catalog() {
            assert!(!info.drive.is_empty(), "{:?} drive is empty", info.name);
            assert!(!info.observe.is_empty(), "{:?} observe is empty", info.name);
            assert!(
                !info.mechanism.is_empty(),
                "{:?} mechanism is empty",
                info.name
            );
            assert!(
                !info.locator_dialect.is_empty(),
                "{:?} locator_dialect is empty",
                info.name
            );
        }
    }

    #[test]
    fn get_returns_the_catalog_entry_for_each_surface() {
        for info in catalog() {
            assert_eq!(get(info.name).as_ref(), Some(&info));
        }
    }

    #[test]
    fn get_cli_returns_the_cli_entry() {
        assert_eq!(get(Surface::Cli).map(|info| info.name), Some(Surface::Cli));
    }

    /// Golden: pin every field of every entry to its `ideas/expect.md` value, so
    /// a silent data edit — a typo or a swapped column — fails here instead of
    /// slipping past the self-referential `surfaces list` / `get` round-trips
    /// (which re-derive their expectations from the catalog under test).
    #[test]
    fn catalog_matches_the_design_tables() {
        let expected: Vec<SurfaceInfo> = [
            (
                Surface::Cli,
                "run argv",
                "stdout/stderr/exit/files",
                "std process",
                "stream regex-capture / json-path if JSON / `exit`",
            ),
            (
                Surface::Http,
                "issue request",
                "status/headers/body",
                "an HTTP client",
                "`status` / `header:<name>` / json-path",
            ),
            (
                Surface::Browser,
                "press/type by `role[name=…]`",
                "snapshot the a11y tree",
                "CDP `Accessibility` + `Input` via `chromiumoxide` (pure Rust, no Node)",
                "`role[name=…]` + tree relationship (`within` / `ancestor`)",
            ),
            (
                Surface::Gui,
                "press/type by `role[name=…]`",
                "snapshot the a11y tree",
                "AX (macOS `AXUIElement`) · UIA (Windows `IUIAutomation`) · AT-SPI (Linux `atspi`+`zbus`)",
                "`role[name=…]` + tree relationship (`within` / `ancestor`)",
            ),
            (
                Surface::File,
                "write",
                "files/dirs/content",
                "the filesystem",
                "path + content (+ sub-locator if structured)",
            ),
            (
                Surface::Db,
                "run statements",
                "rows/tables",
                "a DB client",
                "a SQL query + projection",
            ),
        ]
        .into_iter()
        .map(|(name, drive, observe, mechanism, locator_dialect)| SurfaceInfo {
            name,
            drive: drive.to_string(),
            observe: observe.to_string(),
            mechanism: mechanism.to_string(),
            locator_dialect: locator_dialect.to_string(),
            deterministic: true,
        })
        .collect();
        assert_eq!(catalog(), expected);
    }
}
