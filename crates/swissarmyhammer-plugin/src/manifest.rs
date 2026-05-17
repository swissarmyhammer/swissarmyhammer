//! The plugin manifest: `plugin.json`, the declaration every plugin ships.
//!
//! A plugin bundle is a directory; its `plugin.json` file declares the
//! plugin's identity ([`id`](Manifest::id)), its display [`name`](Manifest::name)
//! and [`version`](Manifest::version), the relative path to its
//! [`entry`](Manifest::entry) TypeScript module, and the set of server names it
//! will [`provides`](Manifest::provides) — register at load time.
//!
//! # Identity follows the manifest, not the directory
//!
//! The on-disk directory name a plugin lives in does **not** have to match its
//! [`id`](Manifest::id). The manifest's `id` is authoritative for identity
//! across layers: discovery keys plugins by `id`, so a plugin can be shadowed
//! by — or shadow — a copy in another layer regardless of what each layer's
//! directory happens to be called.
//!
//! # `provides` is a contract
//!
//! [`provides`](Manifest::provides) is the set of server names the plugin
//! promises to register. The host enforces it two ways: a `provides` name that
//! collides with a reserved host server name is rejected before the plugin
//! loads, and a `this.register(name, …)` for a `name` not in `provides` is
//! rejected at load time. Both checks live on top of the parsed manifest.
//!
//! # `entry` is sandboxed
//!
//! [`entry`](Manifest::entry) is plugin-authored, so it cannot be trusted as a
//! bare filesystem path: a manifest with `"entry": "../../../etc/passwd"` — or
//! an absolute path — would escape the plugin's bundle directory. The manifest
//! is the single place this is validated: [`Manifest::resolve_entry`]
//! canonicalizes the bundle root and the joined entry path and rejects any
//! `entry` that is absolute or resolves outside the bundle. The host always
//! resolves a manifest's entry through that method, so the entry the runtime
//! evaluates is genuinely contained — the module loader can treat it as
//! trusted because it was checked here. This mirrors the canonicalize-and-
//! `starts_with` containment rule the module loader applies to relative
//! imports.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// The filename of a plugin bundle's manifest.
///
/// Every plugin directory discovered through `swissarmyhammer-directory`
/// carries exactly this file at its root; a directory without it is not a
/// plugin and is skipped by discovery.
pub const MANIFEST_FILE: &str = "plugin.json";

/// A plugin's `plugin.json` manifest, deserialized.
///
/// The manifest is the single source of truth for a plugin's identity and the
/// servers it provides. It is parsed once at discovery time; the host then
/// carries it alongside the loaded plugin so the register path can enforce
/// [`provides`](Self::provides).
///
/// Every field is required: a `plugin.json` missing any of them fails to parse
/// with a clear, field-naming error (see [`Manifest::load`]). The struct
/// rejects unknown fields so a typo in a key — `provide` for `provides` — is a
/// loud error rather than a silently dropped declaration.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The plugin's stable identity, authoritative across layers.
    ///
    /// Discovery keys plugins by this `id`, not by their directory name, so the
    /// same `id` in two layers is recognized as two copies of one plugin and
    /// the precedence rules pick a single winner.
    pub id: String,

    /// The plugin's human-readable display name.
    pub name: String,

    /// The plugin's version string.
    ///
    /// Carried verbatim; the platform does not parse or compare it here.
    pub version: String,

    /// Path to the entry TypeScript module, relative to the plugin directory.
    ///
    /// May name a nested path such as `src/plugin.ts`. The host joins it onto
    /// the plugin directory to locate the module it evaluates and whose `load`
    /// export it runs.
    pub entry: String,

    /// The server names this plugin will register at load time.
    ///
    /// The plugin's `load()` may only `this.register(name, …)` for a `name`
    /// listed here; the host rejects any other registration. A name that
    /// collides with a reserved host server name is rejected before the plugin
    /// is loaded at all.
    pub provides: Vec<String>,
}

impl Manifest {
    /// Loads and parses the `plugin.json` at the root of `plugin_dir`.
    ///
    /// # Parameters
    ///
    /// - `plugin_dir` — the plugin's bundle directory; its root must contain a
    ///   [`MANIFEST_FILE`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Manifest`] when the file cannot be read, when its JSON
    /// is malformed, or when a required field is missing — in every case the
    /// message names the offending plugin directory and, for a parse failure,
    /// the precise problem (`missing field 'entry'`, a syntax error, an unknown
    /// key). A plugin with an unreadable or invalid manifest is never loaded,
    /// so a broken manifest fails loudly at discovery rather than mid-load.
    pub fn load(plugin_dir: &Path) -> Result<Self> {
        let manifest_path = plugin_dir.join(MANIFEST_FILE);
        let raw = std::fs::read_to_string(&manifest_path).map_err(|error| {
            Error::Manifest(format!(
                "could not read {} for plugin at {}: {error}",
                MANIFEST_FILE,
                plugin_dir.display()
            ))
        })?;
        Self::parse(&raw, plugin_dir)
    }

    /// Parses a `plugin.json` document from its raw text.
    ///
    /// Split out from [`load`](Self::load) so the parsing — and its
    /// field-naming error behavior — can be unit-tested without touching the
    /// filesystem.
    ///
    /// # Parameters
    ///
    /// - `raw` — the `plugin.json` document text.
    /// - `plugin_dir` — the plugin directory the document came from, used only
    ///   to make a parse error name the offending plugin.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Manifest`] when `raw` is not valid JSON, is missing a
    /// required field, or carries an unknown key.
    pub fn parse(raw: &str, plugin_dir: &Path) -> Result<Self> {
        serde_json::from_str(raw).map_err(|error| {
            Error::Manifest(format!(
                "invalid {} for plugin at {}: {error}",
                MANIFEST_FILE,
                plugin_dir.display()
            ))
        })
    }

    /// Resolves the manifest's [`entry`](Self::entry) to a path proven to be
    /// contained within the plugin's bundle directory.
    ///
    /// The `entry` field is plugin-authored and so cannot be trusted as a raw
    /// filesystem path. This method is the single place that containment is
    /// enforced: it joins `entry` onto `plugin_dir`, canonicalizes both the
    /// bundle root and the joined path — collapsing `..` segments and resolving
    /// symlinks — and rejects any `entry` that is absolute or that resolves
    /// outside the bundle root. The check mirrors the canonicalize-and-
    /// `starts_with` sandbox rule the module loader applies to a plugin's
    /// relative imports, so a manifest cannot reach outside its bundle the way
    /// an import cannot.
    ///
    /// # Parameters
    ///
    /// - `plugin_dir` — the plugin's bundle directory; the `entry` path is
    ///   resolved relative to it and must stay within it.
    ///
    /// # Returns
    ///
    /// The canonicalized, bundle-contained absolute path of the entry module.
    /// The host hands this path to the runtime, so the module it evaluates is
    /// genuinely inside the bundle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Manifest`] — naming the offending plugin — when `entry`
    /// is an absolute path, when the joined entry path cannot be canonicalized
    /// (for example, the file does not exist), or when the canonicalized path
    /// escapes the canonicalized bundle root.
    pub fn resolve_entry(&self, plugin_dir: &Path) -> Result<PathBuf> {
        // An absolute `entry` is rejected outright: even when it happens to
        // point inside the bundle, it is not the relative-within-bundle path
        // the contract requires, and accepting it would normalize a shape that
        // exists only to escape.
        if Path::new(&self.entry).is_absolute() {
            return Err(Error::Manifest(format!(
                "plugin '{}' at {}: manifest entry '{}' must be a path relative \
                 to the plugin directory, not an absolute path",
                self.id,
                plugin_dir.display(),
                self.entry,
            )));
        }

        // Canonicalize the bundle root so the containment check below compares
        // like-for-like: both sides have their symlinks and `..` collapsed.
        let bundle_root = plugin_dir.canonicalize().map_err(|error| {
            Error::Manifest(format!(
                "plugin '{}': cannot resolve bundle directory {}: {error}",
                self.id,
                plugin_dir.display(),
            ))
        })?;

        // Canonicalizing the joined entry path collapses any `..` traversal,
        // so an `entry` like `../escape.ts` resolves to its real location and
        // the `starts_with` check below catches an escape.
        let entry_path = bundle_root.join(&self.entry);
        let canonical_entry = entry_path.canonicalize().map_err(|error| {
            Error::Manifest(format!(
                "plugin '{}' at {}: cannot resolve manifest entry '{}': {error}",
                self.id,
                plugin_dir.display(),
                self.entry,
            ))
        })?;

        if !canonical_entry.starts_with(&bundle_root) {
            return Err(Error::Manifest(format!(
                "plugin '{}' at {}: manifest entry '{}' escapes the plugin \
                 bundle directory",
                self.id,
                plugin_dir.display(),
                self.entry,
            )));
        }

        Ok(canonical_entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A complete, valid `plugin.json` document.
    const VALID: &str = r#"{
        "id": "weather-plugin",
        "name": "Weather",
        "version": "1.0.0",
        "entry": "src/plugin.ts",
        "provides": ["weather"]
    }"#;

    /// A valid manifest parses, and every field is carried through verbatim.
    #[test]
    fn a_valid_manifest_parses_with_every_field() {
        let manifest = Manifest::parse(VALID, &PathBuf::from("/plugins/weather-dir"))
            .expect("a complete manifest should parse");
        assert_eq!(manifest.id, "weather-plugin");
        assert_eq!(manifest.name, "Weather");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.entry, "src/plugin.ts");
        assert_eq!(manifest.provides, vec!["weather".to_string()]);
    }

    /// A manifest missing a required field fails with a clear error that names
    /// the missing field.
    #[test]
    fn a_manifest_missing_a_required_field_errors_clearly() {
        // `entry` is omitted.
        let incomplete = r#"{
            "id": "weather-plugin",
            "name": "Weather",
            "version": "1.0.0",
            "provides": ["weather"]
        }"#;
        let error = Manifest::parse(incomplete, &PathBuf::from("/plugins/weather-dir"))
            .expect_err("a manifest missing `entry` must fail to parse");
        let message = error.to_string();
        assert!(
            message.contains("entry"),
            "the error must name the missing `entry` field, got: {message}"
        );
        assert!(
            message.contains("weather-dir"),
            "the error must name the offending plugin directory, got: {message}"
        );
    }

    /// The disk directory name need not match the manifest `id`: parsing keys
    /// off the manifest, so a mismatch is not an error and `id` is carried as
    /// authoritative.
    #[test]
    fn directory_name_need_not_match_the_manifest_id() {
        // The manifest came from a directory called `some-other-dir`, but the
        // manifest's own `id` is `weather-plugin`.
        let manifest = Manifest::parse(VALID, &PathBuf::from("/plugins/some-other-dir"))
            .expect("a manifest from a differently-named directory should still parse");
        assert_eq!(
            manifest.id, "weather-plugin",
            "identity follows the manifest `id`, not the directory name"
        );
    }

    /// An unknown key — a typo of a real field — is rejected loudly rather than
    /// silently dropped.
    #[test]
    fn an_unknown_key_is_rejected() {
        // `provide` is a typo of `provides`.
        let typo = r#"{
            "id": "weather-plugin",
            "name": "Weather",
            "version": "1.0.0",
            "entry": "src/plugin.ts",
            "provide": ["weather"]
        }"#;
        let error = Manifest::parse(typo, &PathBuf::from("/plugins/weather-dir"))
            .expect_err("a typo'd key must fail to parse");
        assert!(
            error.to_string().contains("provide"),
            "the error must name the unknown key, got: {error}"
        );
    }

    /// Malformed JSON fails with a manifest error rather than a panic.
    #[test]
    fn malformed_json_errors_as_a_manifest_error() {
        let error = Manifest::parse("{ not json", &PathBuf::from("/plugins/broken"))
            .expect_err("malformed JSON must fail to parse");
        assert!(
            matches!(error, Error::Manifest(_)),
            "a malformed manifest must surface as Error::Manifest, got: {error:?}"
        );
    }

    /// Builds a [`Manifest`] with the given `entry` and a fixed identity, for
    /// the entry-containment tests below.
    fn manifest_with_entry(entry: &str) -> Manifest {
        Manifest {
            id: "probe".to_string(),
            name: "Probe".to_string(),
            version: "1.0.0".to_string(),
            entry: entry.to_string(),
            provides: vec!["probe-server".to_string()],
        }
    }

    /// A well-formed `entry` — a real file inside the bundle, including a
    /// nested path — resolves to a canonical path within the bundle root.
    #[test]
    fn a_contained_entry_resolves_within_the_bundle() {
        let bundle = tempfile::TempDir::new().expect("temp bundle dir");
        std::fs::create_dir_all(bundle.path().join("src")).expect("src dir");
        std::fs::write(bundle.path().join("src").join("plugin.ts"), "// entry")
            .expect("entry file");

        let manifest = manifest_with_entry("src/plugin.ts");
        let resolved = manifest
            .resolve_entry(bundle.path())
            .expect("a contained entry should resolve");

        let bundle_root = bundle.path().canonicalize().expect("canonical bundle root");
        assert!(
            resolved.starts_with(&bundle_root),
            "the resolved entry must stay inside the bundle root, got: {}",
            resolved.display()
        );
        assert!(
            resolved.is_file(),
            "the resolved entry must be the real entry file"
        );
    }

    /// An `entry` that traverses out of the bundle with `..` is rejected with a
    /// clear, plugin-naming manifest error.
    #[test]
    fn an_entry_escaping_the_bundle_via_dotdot_is_rejected() {
        let parent = tempfile::TempDir::new().expect("temp parent dir");
        let bundle = parent.path().join("bundle");
        std::fs::create_dir_all(&bundle).expect("bundle dir");
        // The would-be escape target: a sibling file outside the bundle.
        std::fs::write(parent.path().join("escape.ts"), "// outside the bundle")
            .expect("escape target file");

        let manifest = manifest_with_entry("../escape.ts");
        let error = manifest
            .resolve_entry(&bundle)
            .expect_err("an entry escaping the bundle must be rejected");
        assert!(
            matches!(error, Error::Manifest(_)),
            "an escaping entry must surface as Error::Manifest, got: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("escapes the plugin bundle"),
            "the error must explain the escape, got: {message}"
        );
        assert!(
            message.contains("probe"),
            "the error must name the offending plugin, got: {message}"
        );
    }

    /// An absolute `entry` path is rejected even though it never traverses with
    /// `..`: an absolute path is not the relative-within-bundle shape required.
    #[test]
    fn an_absolute_entry_is_rejected() {
        let bundle = tempfile::TempDir::new().expect("temp bundle dir");
        std::fs::write(bundle.path().join("entry.ts"), "// entry").expect("entry file");

        // An absolute path that exists — `/etc/hosts` on a POSIX host — still
        // must be rejected: it is absolute, not relative within the bundle.
        let manifest = manifest_with_entry("/etc/hosts");
        let error = manifest
            .resolve_entry(bundle.path())
            .expect_err("an absolute entry must be rejected");
        let message = error.to_string();
        assert!(
            message.contains("absolute"),
            "the error must explain the absolute-path rejection, got: {message}"
        );
        assert!(
            message.contains("probe"),
            "the error must name the offending plugin, got: {message}"
        );
    }
}
