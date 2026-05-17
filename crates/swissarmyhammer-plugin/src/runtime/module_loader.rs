//! The module loader for multi-file plugin bundles.
//!
//! A plugin is not a single file: it has an entry module that imports helpers,
//! and it imports the host SDK. To run such a plugin without a bundler, the
//! V8 isolate needs a [`deno_core::ModuleLoader`] that can resolve and fetch
//! each imported module. [`PluginModuleLoader`] is that loader.
//!
//! # The three import kinds
//!
//! Every import a plugin makes falls into exactly one of three kinds, and the
//! loader treats each differently:
//!
//! - **Relative imports** (`./util`, `../shared/foo`) resolve against the
//!   importing module's directory. Each resolved module is read from disk and
//!   transpiled by [`transpile`](super::transpile) — the same TypeScript-to-
//!   JavaScript path the entry module takes. A relative import is **sandboxed**:
//!   its canonicalized path must stay inside the plugin's bundle directory, so
//!   a plugin cannot reach files outside its own bundle. This mirrors the
//!   sandbox rule in `swissarmyhammer-js`'s `SandboxedModuleLoader`.
//! - **Bare imports** (`lodash`, `zod`) are **rejected**. The host is not an
//!   npm client: a plugin author is expected to bundle third-party npm
//!   dependencies into their own bundle. A bare specifier fails with a clear,
//!   specific error rather than a panic or a silent empty module.
//! - **`@swissarmyhammer/*` imports** resolve to host-provided **virtual
//!   modules** served from memory — never from disk. `@swissarmyhammer/plugin`
//!   is the plugin SDK — its real source is embedded in [`crate::sdk`] and
//!   transpiled into the virtual-module table when the loader is built.
//!   `@swissarmyhammer/app` is generated app-binding code, still a no-op stub
//!   until app codegen lands. This loader provides the resolution and
//!   in-memory serving plumbing for both.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deno_core::error::ModuleLoaderError;
use deno_core::{
    FastString, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;

use super::transpile::transpile_typescript;

/// The synthetic URL scheme that host-provided virtual modules resolve to.
///
/// An `@swissarmyhammer/<name>` import is rewritten in [`PluginModuleLoader::resolve`]
/// to a `swissarmyhammer:<name>` URL. The [`PluginModuleLoader::load`] step then
/// recognizes that scheme and serves the module body from memory rather than
/// reading the filesystem.
const VIRTUAL_SCHEME: &str = "swissarmyhammer";

/// The npm-style scope prefix that marks a host-provided virtual module.
///
/// Any import whose specifier begins with `@swissarmyhammer/` is a host
/// built-in; everything else under that scope would still be routed here and
/// rejected by [`virtual_module_name`] if it has no registered virtual module.
const HOST_SCOPE_PREFIX: &str = "@swissarmyhammer/";

/// A `deno_core::ModuleLoader` for plugin bundles.
///
/// The loader resolves relative imports against a configurable plugin-bundle
/// root, serves `@swissarmyhammer/*` specifiers from an in-memory virtual-module
/// table, and rejects bare npm specifiers. See the [module documentation] for
/// the full resolution contract.
///
/// `deno_core` fixes the [`ModuleLoader`] when the `JsRuntime` is constructed,
/// so the bundle root cannot be a constructor argument: it is held behind a
/// [`RefCell`] and set per plugin load via [`PluginModuleLoader::set_bundle_root`].
/// `deno_core` only ever calls the loader from the runtime's single worker
/// thread, so a plain `RefCell` (no locking, `!Sync`) is sufficient.
///
/// [module documentation]: self
pub struct PluginModuleLoader {
    /// The canonicalized plugin-bundle root, or `None` until a plugin is loaded.
    ///
    /// While `None`, every relative import is rejected because there is no
    /// directory to resolve against and contain to. It is set to the
    /// canonicalized bundle directory by [`PluginModuleLoader::set_bundle_root`]
    /// at the start of each plugin load.
    bundle_root: RefCell<Option<PathBuf>>,

    /// The host-provided virtual modules, keyed by their `swissarmyhammer:<name>`
    /// URL string.
    ///
    /// Populated once at construction by [`virtual_modules`]; immutable
    /// thereafter. A `@swissarmyhammer/*` import resolves into this table. The
    /// values are JavaScript ready for V8 — the `@swissarmyhammer/plugin` entry
    /// is the SDK source transpiled from TypeScript at construction time.
    virtual_modules: HashMap<String, String>,
}

impl PluginModuleLoader {
    /// Create a loader with no bundle root and the host virtual modules loaded.
    ///
    /// Until [`PluginModuleLoader::set_bundle_root`] is called, relative imports
    /// are rejected; `@swissarmyhammer/*` imports work immediately because the
    /// virtual-module table is populated here.
    pub fn new() -> Self {
        Self {
            bundle_root: RefCell::new(None),
            virtual_modules: virtual_modules(),
        }
    }

    /// Set the plugin-bundle root that relative imports resolve against.
    ///
    /// The path is canonicalized so the sandbox containment check in
    /// [`PluginModuleLoader::resolve`] compares like-for-like (symlinks and
    /// `..` segments collapsed). Called at the start of each plugin load.
    ///
    /// # Arguments
    ///
    /// * `root` - The plugin's bundle directory.
    ///
    /// # Errors
    ///
    /// Returns an error message if `root` cannot be canonicalized — for
    /// example, when the directory does not exist.
    pub fn set_bundle_root(&self, root: &Path) -> Result<(), String> {
        let canonical = root
            .canonicalize()
            .map_err(|e| format!("cannot resolve plugin bundle directory: {e}"))?;
        *self.bundle_root.borrow_mut() = Some(canonical);
        Ok(())
    }

    /// Resolve a filesystem specifier to a sandboxed `file://` URL.
    ///
    /// Two specifier shapes route here: a **relative** specifier (`./`, `../`),
    /// which resolves against the importing module's directory (taken from
    /// `referrer`) — or the bundle root for the entry module — and an absolute
    /// **`file://`** specifier, which is already a concrete path (this is how
    /// `deno_core` re-presents the entry module's own URL). Either way the
    /// resolved path is canonicalized and verified to stay inside the
    /// canonicalized bundle root; a specifier that escapes — via an absolute
    /// path or `..` traversal — is rejected. This is the same containment rule
    /// as `swissarmyhammer-js`'s `SandboxedModuleLoader`.
    fn resolve_relative(&self, specifier: &str, referrer: &str) -> Result<ModuleSpecifier, String> {
        let root = self
            .bundle_root
            .borrow()
            .clone()
            .ok_or_else(|| "plugin bundle directory is not configured".to_string())?;

        // A `file://` specifier is already an absolute path, so it has no
        // resolution base; otherwise a relative specifier resolves against the
        // importing module's directory, falling back to the bundle root for the
        // entry module (whose referrer is not a module file).
        let requested = if specifier.starts_with("file://") {
            ModuleSpecifier::parse(specifier)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .ok_or_else(|| format!("invalid file:// import URL: {specifier}"))?
        } else {
            let resolution_dir = referrer_directory(referrer).unwrap_or_else(|| root.clone());
            resolution_dir.join(specifier)
        };

        // Canonicalize to collapse `..` segments and resolve symlinks, then
        // verify the result is still inside the bundle root.
        let canonical = requested
            .canonicalize()
            .map_err(|e| format!("cannot resolve import '{specifier}': {e}"))?;
        if !canonical.starts_with(&root) {
            return Err(format!(
                "import '{specifier}' escapes the plugin bundle directory"
            ));
        }

        ModuleSpecifier::from_file_path(&canonical)
            .map_err(|_| format!("cannot build a file URL for import '{specifier}'"))
    }

    /// Resolve an `@swissarmyhammer/*` specifier to its virtual-module URL.
    ///
    /// The specifier is rewritten to a `swissarmyhammer:<name>` URL, which
    /// [`PluginModuleLoader::load`] recognizes and serves from memory. A
    /// specifier under the `@swissarmyhammer/` scope that has no registered
    /// virtual module is rejected.
    fn resolve_virtual(&self, specifier: &str) -> Result<ModuleSpecifier, String> {
        let name = virtual_module_name(specifier)
            .ok_or_else(|| format!("unknown host module '{specifier}'"))?;
        let url = format!("{VIRTUAL_SCHEME}:{name}");
        if !self.virtual_modules.contains_key(&url) {
            return Err(format!("unknown host module '{specifier}'"));
        }
        ModuleSpecifier::parse(&url)
            .map_err(|e| format!("cannot build a URL for host module '{specifier}': {e}"))
    }

    /// Serve a host virtual module from the in-memory table.
    ///
    /// The `specifier` has already passed [`PluginModuleLoader::resolve`], so it
    /// is a `swissarmyhammer:<name>` URL known to be in the table.
    fn load_virtual(&self, specifier: &ModuleSpecifier) -> Result<ModuleSource, JsErrorBox> {
        let code = self
            .virtual_modules
            .get(specifier.as_str())
            .ok_or_else(|| {
                JsErrorBox::generic(format!("host module '{specifier}' is not registered"))
            })?;
        Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleSourceCode::String(FastString::from(code.clone())),
            specifier,
            None,
        ))
    }

    /// Read a plugin module from disk and transpile it to JavaScript.
    ///
    /// The `specifier` is a `file://` URL produced by
    /// [`PluginModuleLoader::resolve`]. For a plugin *import* that URL has
    /// passed the sandbox containment check, so it is known to be inside the
    /// bundle root. For the entry/main module it has not: `resolve` returns the
    /// `MainModule` URL unchecked because the entry path is host-derived
    /// (`bundle_dir.join(entry_file)`) and trusted, not plugin-chosen. The file
    /// is read, transpiled via [`transpile_typescript`] (the same path the
    /// entry module takes), and returned as a JavaScript [`ModuleSource`].
    fn load_relative(&self, specifier: &ModuleSpecifier) -> Result<ModuleSource, JsErrorBox> {
        let path = specifier
            .to_file_path()
            .map_err(|()| JsErrorBox::generic(format!("invalid file URL '{specifier}'")))?;
        let source = std::fs::read_to_string(&path).map_err(|e| {
            JsErrorBox::generic(format!("failed to read module {}: {e}", path.display()))
        })?;
        let transpiled = transpile_typescript(specifier, &source)
            .map_err(|e| JsErrorBox::generic(format!("failed to transpile {specifier}: {e}")))?;
        Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleSourceCode::String(transpiled.code.into()),
            specifier,
            None,
        ))
    }
}

impl Default for PluginModuleLoader {
    /// Create a loader with no bundle root, equivalent to [`PluginModuleLoader::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleLoader for PluginModuleLoader {
    /// Resolve an import specifier to one of the three import kinds.
    ///
    /// The entry module is special: `deno_core` resolves it with
    /// [`ResolutionKind::MainModule`], and its specifier is the host-chosen
    /// entry URL — not something plugin code imported — so it passes straight
    /// through without a sandbox check. Every *import* a module makes
    /// (`ResolutionKind::Import` / `DynamicImport`) is classified instead: a
    /// `@swissarmyhammer/*` specifier resolves to a virtual-module URL; a
    /// relative specifier (`./`, `../`) resolves to a sandboxed `file://` URL;
    /// the `swissarmyhammer:` scheme used by an already-resolved virtual module
    /// passes straight through. Any other specifier is a bare npm import and is
    /// rejected with a clear error. Every failure is surfaced as a `JsErrorBox`,
    /// which V8 reports to the importing module.
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        // The entry module's own URL is host-chosen, not plugin-imported, so it
        // is not sandbox-checked here. Its relative imports still are: they are
        // resolved with `ResolutionKind::Import` and the entry's `file://`
        // referrer, and fall through to the classification below.
        if kind == ResolutionKind::MainModule {
            return ModuleSpecifier::parse(specifier).map_err(|e| {
                JsErrorBox::generic(format!("invalid entry module URL '{specifier}': {e}"))
            });
        }
        if specifier.starts_with(HOST_SCOPE_PREFIX) {
            return self.resolve_virtual(specifier).map_err(JsErrorBox::generic);
        }
        if specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with("file://")
        {
            return self
                .resolve_relative(specifier, referrer)
                .map_err(JsErrorBox::generic);
        }
        // An already-resolved virtual module's URL passes straight back: it
        // never needs re-resolution, but parsing it keeps the loader total.
        if specifier.starts_with(&format!("{VIRTUAL_SCHEME}:")) {
            return ModuleSpecifier::parse(specifier).map_err(|e| {
                JsErrorBox::generic(format!("invalid host module URL '{specifier}': {e}"))
            });
        }
        // An absolute filesystem path is classified explicitly so it gets an
        // honest diagnosis: without this check it would fall through to the
        // bare-import branch below and be rejected as a phantom npm package.
        // It is still rejected — a plugin import must be relative or
        // `@swissarmyhammer/*` — just with the correct reason. This mirrors the
        // up-front guard in `swissarmyhammer-js`'s `SandboxedModuleLoader`.
        if specifier.starts_with('/') || specifier.starts_with('\\') {
            return Err(JsErrorBox::generic(format!(
                "absolute import path rejected: '{specifier}' — plugin imports \
                 must be relative ('./', '../') or '@swissarmyhammer/*'"
            )));
        }
        Err(JsErrorBox::generic(format!(
            "bare import '{specifier}' is not resolvable: the plugin host does not \
             resolve npm packages — bundle '{specifier}' into your plugin bundle yourself"
        )))
    }

    /// Load a previously resolved module's source.
    ///
    /// A `swissarmyhammer:` URL is served from the in-memory virtual-module
    /// table; a `file://` URL is read from disk and transpiled. The specifier
    /// has already passed [`PluginModuleLoader::resolve`], so no further
    /// sandbox check is needed here.
    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let result = if module_specifier.scheme() == VIRTUAL_SCHEME {
            self.load_virtual(module_specifier)
        } else if module_specifier.scheme() == "file" {
            self.load_relative(module_specifier)
        } else {
            Err(JsErrorBox::generic(format!(
                "cannot load module '{module_specifier}': unsupported scheme"
            )))
        };
        ModuleLoadResponse::Sync(result)
    }
}

/// Extract the directory of an importing module from its referrer URL.
///
/// Returns the parent directory of the referring module when `referrer` is a
/// parseable `file://` URL pointing at a file. Returns `None` for any other
/// referrer (the entry module's synthetic referrer, a `swissarmyhammer:` URL),
/// so the caller falls back to the bundle root for the entry module.
///
/// This is intentionally a verbatim twin of the private `referrer_directory`
/// in `swissarmyhammer-js/src/lib.rs`. The duplication is deliberate: the two
/// crates are independent and neither depends on the other, and a ~6-line path
/// helper does not warrant a shared crate. Keep the two copies in sync by hand
/// if either is changed.
fn referrer_directory(referrer: &str) -> Option<PathBuf> {
    if !referrer.starts_with("file://") {
        return None;
    }
    let path = ModuleSpecifier::parse(referrer).ok()?.to_file_path().ok()?;
    path.parent().map(PathBuf::from)
}

/// Map an `@swissarmyhammer/<name>` specifier to its bare virtual-module name.
///
/// Returns `Some("plugin")` for `@swissarmyhammer/plugin`, and `None` for a
/// specifier that is not under the `@swissarmyhammer/` scope or names a
/// nested path the host does not serve.
fn virtual_module_name(specifier: &str) -> Option<&str> {
    let name = specifier.strip_prefix(HOST_SCOPE_PREFIX)?;
    // Only a single bare segment is a host module; `@swissarmyhammer/plugin/x`
    // is not something the host serves.
    if name.is_empty() || name.contains('/') {
        return None;
    }
    Some(name)
}

/// Build the host virtual-module table, keyed by `swissarmyhammer:<name>` URL.
///
/// Two modules are provided:
///
/// - `@swissarmyhammer/plugin` — the plugin SDK. Its source is embedded in
///   [`crate::sdk`] as TypeScript; it is transpiled to JavaScript here so the
///   table holds V8-ready code.
/// - `@swissarmyhammer/app` — generated app-binding types. Codegen is a
///   separate task; this is a deliberate no-op stub.
///
/// # Panics
///
/// Panics if the embedded SDK source fails to transpile. The SDK source is
/// compiled into the binary, so a transpile failure is a build-time bug in the
/// crate, not a recoverable runtime condition.
fn virtual_modules() -> HashMap<String, String> {
    let mut modules = HashMap::new();
    modules.insert(format!("{VIRTUAL_SCHEME}:plugin"), transpiled_sdk());
    modules.insert(
        format!("{VIRTUAL_SCHEME}:app"),
        APP_BINDINGS_STUB.to_string(),
    );
    modules
}

/// Transpile the embedded `@swissarmyhammer/plugin` SDK source to JavaScript.
///
/// The SDK ([`crate::sdk::SDK_PLUGIN_SOURCE`]) is TypeScript; the virtual
/// module table must hold JavaScript V8 can evaluate directly. The transpiled
/// code carries an inline source map so plugin stack traces that pass through
/// the SDK report original TypeScript positions.
///
/// # Panics
///
/// Panics if the embedded SDK source cannot be transpiled — that is a
/// build-time bug in this crate, since the source ships inside the binary.
fn transpiled_sdk() -> String {
    let specifier = ModuleSpecifier::parse(&format!("{VIRTUAL_SCHEME}:plugin"))
        .expect("the SDK virtual-module URL must be a valid specifier");
    transpile_typescript(&specifier, crate::sdk::SDK_PLUGIN_SOURCE)
        .expect("the embedded @swissarmyhammer/plugin SDK source must transpile")
        .code
}

/// Stub body for the `@swissarmyhammer/app` generated-bindings virtual module.
///
/// A deliberate no-op: `@swissarmyhammer/app` is generated by app codegen,
/// which is a separate task. The stub keeps a plugin that imports it loadable.
const APP_BINDINGS_STUB: &str = "\
// @swissarmyhammer/app — generated app bindings stub.
// App-binding codegen is a separate task; this no-op module keeps a plugin
// that imports it loadable until generated bindings land.
export const __appStub = true;
";

#[cfg(test)]
mod tests {
    use super::*;

    /// `virtual_module_name` accepts a single bare scope segment only.
    #[test]
    fn virtual_module_name_accepts_single_segment() {
        assert_eq!(
            virtual_module_name("@swissarmyhammer/plugin"),
            Some("plugin")
        );
        assert_eq!(virtual_module_name("@swissarmyhammer/app"), Some("app"));
        assert_eq!(virtual_module_name("@swissarmyhammer/plugin/deep"), None);
        assert_eq!(virtual_module_name("@swissarmyhammer/"), None);
        assert_eq!(virtual_module_name("lodash"), None);
    }

    /// A bare specifier is rejected with a host-specific error that names the
    /// specifier and explains the bundling expectation.
    #[test]
    fn bare_import_is_rejected_with_clear_error() {
        let loader = PluginModuleLoader::new();
        let error = loader
            .resolve("lodash", "file:///plugin/entry.ts", ResolutionKind::Import)
            .expect_err("a bare import must not resolve");
        let message = error.to_string();
        assert!(
            message.contains("lodash"),
            "the error should name the bare specifier, got: {message}"
        );
        assert!(
            message.contains("bundle"),
            "the error should explain that the author must bundle it, got: {message}"
        );
    }

    /// An absolute-path specifier is rejected with the absolute-path error,
    /// not misdiagnosed as a bare npm import.
    #[test]
    fn absolute_import_path_is_rejected_as_absolute() {
        let loader = PluginModuleLoader::new();
        for specifier in ["/etc/passwd", "\\windows\\system32"] {
            let error = loader
                .resolve(specifier, "file:///plugin/entry.ts", ResolutionKind::Import)
                .expect_err("an absolute import path must not resolve");
            let message = error.to_string();
            assert!(
                message.contains("absolute import path rejected"),
                "the error should diagnose an absolute path, got: {message}"
            );
            assert!(
                message.contains(specifier),
                "the error should name the rejected specifier, got: {message}"
            );
            assert!(
                !message.contains("npm"),
                "an absolute path must not be misdiagnosed as a bare npm import, \
                 got: {message}"
            );
        }
    }

    /// An `@swissarmyhammer/*` specifier resolves to its virtual-module URL.
    #[test]
    fn host_scope_specifier_resolves_to_virtual_url() {
        let loader = PluginModuleLoader::new();
        let resolved = loader
            .resolve(
                "@swissarmyhammer/plugin",
                "file:///plugin/entry.ts",
                ResolutionKind::Import,
            )
            .expect("an @swissarmyhammer/plugin import should resolve");
        assert_eq!(resolved.scheme(), VIRTUAL_SCHEME);
        assert_eq!(resolved.as_str(), "swissarmyhammer:plugin");
    }

    /// An unknown `@swissarmyhammer/*` specifier is rejected, not silently
    /// served as an empty module.
    #[test]
    fn unknown_host_module_is_rejected() {
        let loader = PluginModuleLoader::new();
        let error = loader
            .resolve(
                "@swissarmyhammer/does-not-exist",
                "file:///plugin/entry.ts",
                ResolutionKind::Import,
            )
            .expect_err("an unknown host module must not resolve");
        assert!(
            error.to_string().contains("unknown host module"),
            "got: {error}"
        );
    }

    /// A relative import is rejected when no bundle root has been configured.
    #[test]
    fn relative_import_without_bundle_root_is_rejected() {
        let loader = PluginModuleLoader::new();
        let error = loader
            .resolve(
                "./util.ts",
                "file:///plugin/entry.ts",
                ResolutionKind::Import,
            )
            .expect_err("a relative import needs a configured bundle root");
        assert!(error.to_string().contains("not configured"), "got: {error}");
    }

    /// A relative import inside the bundle resolves to a `file://` URL.
    #[test]
    fn relative_import_inside_bundle_resolves() {
        let bundle = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(bundle.path().join("util.ts"), "export const x = 1;")
            .expect("util.ts written");
        let entry = bundle.path().join("entry.ts");
        std::fs::write(&entry, "import './util.ts';").expect("entry.ts written");

        let loader = PluginModuleLoader::new();
        loader
            .set_bundle_root(bundle.path())
            .expect("bundle root should be set");

        let referrer = ModuleSpecifier::from_file_path(&entry).unwrap();
        let resolved = loader
            .resolve("./util.ts", referrer.as_str(), ResolutionKind::Import)
            .expect("a relative import inside the bundle should resolve");
        assert_eq!(resolved.scheme(), "file");
        assert!(resolved.as_str().ends_with("util.ts"));
    }

    /// A relative import escaping the bundle root is rejected.
    #[test]
    fn relative_import_escaping_bundle_is_rejected() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let bundle = root.path().join("bundle");
        std::fs::create_dir_all(&bundle).expect("bundle dir");
        std::fs::write(root.path().join("outside.ts"), "export const x = 1;")
            .expect("outside.ts written");
        let entry = bundle.join("entry.ts");
        std::fs::write(&entry, "import '../outside.ts';").expect("entry.ts written");

        let loader = PluginModuleLoader::new();
        loader
            .set_bundle_root(&bundle)
            .expect("bundle root should be set");

        let referrer = ModuleSpecifier::from_file_path(&entry).unwrap();
        let error = loader
            .resolve("../outside.ts", referrer.as_str(), ResolutionKind::Import)
            .expect_err("an escaping import must be rejected");
        assert!(
            error.to_string().contains("escapes the plugin bundle"),
            "got: {error}"
        );
    }
}
