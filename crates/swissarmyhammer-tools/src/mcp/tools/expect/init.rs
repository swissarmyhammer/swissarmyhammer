//! `expect init` — scaffold the `.expect/` dot-folder tree.
//!
//! Implements [`Initializable`] for [`ExpectTool`] so `sah init` (and the
//! `expect init` trait verb) creates the single `.expect/` dot folder at the
//! repo root, per `ideas/expect.md` §"expect init". The scaffold is idempotent
//! and never overwrites an existing file.
//!
//! Mirrors the two existing filesystem-scaffold components: the kanban tool's
//! `Initializable` impl (`crate::mcp::tools::kanban`) and the CLI's
//! `ProjectStructure`. Like both, it gates filesystem work to
//! [`InitScope::Project`]/[`InitScope::Local`] and resolves the project root
//! safely (git root, else the working directory) without panicking in a
//! non-git or read-only working directory.
//!
//! `init` also reuses the `detected-projects` machinery
//! ([`detect_projects`]) to pick a sensible default `surface` for the repo and
//! records it in `config.toml`, so the first `expect expectation create` has
//! context to work from. The `config.toml` body is the documented all-defaults
//! template, which round-trips to [`ExpectConfig::default`] (asserted in the
//! tests against that single source of truth).

use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_expect::Surface;
use swissarmyhammer_project_detection::{detect_projects, ProjectType};

use super::ExpectTool;

/// The single dot folder, at the repo root, that holds all `expect` state.
const EXPECT_DIR_NAME: &str = ".expect";

/// The repo-level config file (grading model, embedder, thresholds, policy).
const CONFIG_FILE: &str = "config.toml";
/// The scaffolded README explaining expectations and how to author one.
const README_FILE: &str = "README.md";
/// The worked, ready-to-copy expectation.
const EXAMPLE_FILE: &str = "example.expect.md";
/// The `.expect/.gitignore` reconciled by [`ensure_gitignore`].
const GITIGNORE_FILE: &str = ".gitignore";

/// Subdirectories created under `.expect/`: repo-global specs, committed
/// goldens, and gitignored received runs.
const SCAFFOLD_SUBDIRS: &[&str] = &["expectations", "goldens", "received"];

/// The gitignore entries reconciled into `.expect/.gitignore`.
///
/// Listed explicitly (an exact `received/` directory entry, **not** a blanket
/// `*`) so the `goldens/` tree — committed source — stays tracked. Mirrors the
/// kanban board's `ensure_gitignore_entries` guarantee. Declared at module
/// scope so the tests assert against this single source of truth.
const REQUIRED_GITIGNORE_ENTRIES: &[&str] = &["received/"];

/// The all-defaults `config.toml` body (documented in `ideas/expect.md`
/// §"Config Schema"). A detected-surface header comment is prepended by
/// [`config_contents`]. Round-trips to [`ExpectConfig::default`].
const CONFIG_TEMPLATE: &str = include_str!("templates/config.toml");
/// The scaffolded `README.md`.
const README_TEMPLATE: &str = include_str!("templates/README.md");
/// The scaffolded worked `example.expect.md`.
const EXAMPLE_TEMPLATE: &str = include_str!("templates/example.expect.md");

/// The default `surface` when no project type is detected — the cheapest,
/// always-faithful adapter.
const DEFAULT_SURFACE: Surface = Surface::Cli;

/// Map a detected [`ProjectType`] to its most likely primary [`Surface`].
///
/// A table, not branching: each project type's typical run shape picks one
/// surface (compiled binaries and build systems drive the CLI; web/back-end
/// stacks an HTTP service; GUI toolkits a desktop app). The match is exhaustive
/// so a new `ProjectType` variant is a compile error here rather than a silent
/// default.
fn surface_for_project_type(project_type: ProjectType) -> Surface {
    match project_type {
        ProjectType::Rust
        | ProjectType::Go
        | ProjectType::Python
        | ProjectType::CMake
        | ProjectType::Makefile => Surface::Cli,
        ProjectType::NodeJs
        | ProjectType::JavaMaven
        | ProjectType::JavaGradle
        | ProjectType::CSharp
        | ProjectType::Php => Surface::Http,
        ProjectType::Flutter => Surface::Gui,
    }
}

/// The detected default surfaces for the project rooted at `root`.
///
/// Detection failures (e.g. a path that cannot be canonicalized) are not fatal:
/// they collapse to the [`DEFAULT_SURFACE`] baseline rather than aborting the
/// scaffold. Surfaces are deduplicated and ordered for a stable `config.toml`.
fn detected_surfaces(root: &Path) -> Vec<Surface> {
    let projects = detect_projects(root, None).unwrap_or_default();
    let mut surfaces: Vec<Surface> = projects
        .iter()
        .map(|project| surface_for_project_type(project.project_type))
        .collect();
    surfaces.sort_by_key(|surface| surface_name(*surface));
    surfaces.dedup();
    if surfaces.is_empty() {
        surfaces.push(DEFAULT_SURFACE);
    }
    surfaces
}

/// The lowercase wire name of a [`Surface`], derived from the enum's own serde
/// representation (the source of truth) rather than a re-typed literal.
fn surface_name(surface: Surface) -> String {
    serde_json::to_value(surface)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Build the `config.toml` contents: a detected-surface header comment over the
/// all-defaults [`CONFIG_TEMPLATE`].
fn config_contents(surfaces: &[Surface]) -> String {
    let names = surfaces
        .iter()
        .map(|surface| surface_name(*surface))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "# Detected surface default(s) for new expectations: {names}\n\
         # Set `surface:` in each *.expect.md to one of: cli, http, browser, gui, file, db.\n\
         \n\
         {CONFIG_TEMPLATE}"
    )
}

/// Write `contents` to `path` only when no file is already there.
///
/// The whole scaffold is idempotent: re-running `init` must never clobber an
/// edited `config.toml`, `example.expect.md`, or `README.md`.
fn write_if_absent(path: &Path, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, contents)
}

/// Reconcile `.expect/.gitignore` so every entry in
/// [`REQUIRED_GITIGNORE_ENTRIES`] is present, appending any that are missing.
///
/// Mirrors the kanban board's `ensure_gitignore_entries`: reads whatever is
/// already there, appends only the missing required entries without clobbering
/// existing lines, and rewrites only when something changed. `received/` is
/// listed explicitly (not a blanket `*`) so `goldens/` stays tracked.
fn ensure_gitignore(expect_dir: &Path) -> std::io::Result<()> {
    let gitignore_path = expect_dir.join(GITIGNORE_FILE);
    let existing = std::fs::read_to_string(&gitignore_path).unwrap_or_default();

    let mut lines: Vec<String> = existing.lines().map(|line| line.to_string()).collect();
    let mut changed = false;
    for required in REQUIRED_GITIGNORE_ENTRIES {
        if !lines.iter().any(|line| line.trim() == *required) {
            lines.push((*required).to_string());
            changed = true;
        }
    }

    if changed {
        let mut content = lines.join("\n");
        content.push('\n');
        std::fs::write(&gitignore_path, content)?;
    }
    Ok(())
}

/// Scaffold the `.expect/` tree under an explicit `root`, idempotently.
///
/// Root-explicit so it never reads or mutates the process working directory —
/// [`ExpectTool::init`] resolves the root and passes it here, which also makes
/// the scaffold unit-testable without touching the process CWD. Creates the
/// directory layout, writes the README/example/config (only when absent), and
/// reconciles the `.gitignore`. Surface defaults are detected from `root` and
/// recorded in `config.toml`. Returns the created `.expect/` directory.
fn scaffold_expect_dir(root: &Path) -> std::io::Result<PathBuf> {
    let expect_dir = root.join(EXPECT_DIR_NAME);
    std::fs::create_dir_all(&expect_dir)?;
    for subdir in SCAFFOLD_SUBDIRS {
        std::fs::create_dir_all(expect_dir.join(subdir))?;
    }

    write_if_absent(&expect_dir.join(README_FILE), README_TEMPLATE)?;
    write_if_absent(&expect_dir.join(EXAMPLE_FILE), EXAMPLE_TEMPLATE)?;

    let surfaces = detected_surfaces(root);
    write_if_absent(&expect_dir.join(CONFIG_FILE), &config_contents(&surfaces))?;

    ensure_gitignore(&expect_dir)?;
    Ok(expect_dir)
}

/// Resolve the project root for the scaffold without panicking.
///
/// Prefers the enclosing git repository root; falls back to the process working
/// directory. Returns `None` (rather than `.expect()`-ing) when neither is
/// available — e.g. a bundled GUI app launched with a read-only `/` CWD — so
/// [`ExpectTool::init`] can record a clean `Skipped` result. Heeds the
/// gui-cwd-readonly guidance: never `.expect()` on an env-derived path.
fn resolve_project_root() -> Option<PathBuf> {
    swissarmyhammer_common::utils::find_git_repository_root()
        .or_else(|| std::env::current_dir().ok())
}

impl Initializable for ExpectTool {
    fn name(&self) -> &str {
        <Self as crate::mcp::tool_registry::McpTool>::name(self)
    }

    fn display_name(&self) -> &str {
        "Expectations"
    }

    fn category(&self) -> &str {
        "tools"
    }

    /// Runs at priority 45 — after `ProjectStructure` (40) creates `.sah/` +
    /// `.prompts/`, before the CLAUDE.md preamble (50) — since the `.expect/`
    /// tree is another project-local filesystem scaffold.
    fn priority(&self) -> i32 {
        45
    }

    /// Only applicable to Project and Local scope — never User.
    ///
    /// The `.expect/` tree is project-local runtime state (goldens, received
    /// runs, repo config), so a User-scope install has nothing to scaffold.
    /// Mirrors `ProjectStructure`.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Scaffold the `.expect/` tree. Gated to Project|Local; resolves the root
    /// safely and delegates to the root-explicit [`scaffold_expect_dir`].
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let name = Initializable::name(self);
        if !matches!(scope, InitScope::Project | InitScope::Local) {
            return vec![InitResult::skipped(
                name,
                "expect scaffolding only applies to project/local scope",
            )];
        }

        let root = match resolve_project_root() {
            Some(root) => root,
            None => return vec![InitResult::skipped(name, "Cannot determine project root")],
        };

        match scaffold_expect_dir(&root) {
            Ok(expect_dir) => {
                reporter.emit(&InitEvent::Action {
                    verb: "Created".to_string(),
                    message: format!(".expect/ scaffold at {}", expect_dir.display()),
                });
                vec![InitResult::ok(name, ".expect/ scaffold initialized")]
            }
            Err(e) => vec![InitResult::error(
                name,
                format!("Failed to scaffold .expect/: {e}"),
            )],
        }
    }

    /// Deinit is a no-op that preserves `.expect/`.
    ///
    /// Unlike `.sah/` or kanban merge drivers, `.expect/` holds committed source
    /// (the `goldens/` baselines and authored `*.expect.md` specs), so `deinit`
    /// must never delete it.
    fn deinit(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        vec![InitResult::skipped(
            Initializable::name(self),
            ".expect/ preserved (contains committed goldens and specs)",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_expect::ExpectConfig;

    /// Every scaffolded path from the `ideas/expect.md` §"expect init" tree.
    fn expect_paths(expect_dir: &Path) -> Vec<PathBuf> {
        vec![
            expect_dir.to_path_buf(),
            expect_dir.join(CONFIG_FILE),
            expect_dir.join(README_FILE),
            expect_dir.join(EXAMPLE_FILE),
            expect_dir.join(GITIGNORE_FILE),
            expect_dir.join("expectations"),
            expect_dir.join("goldens"),
            expect_dir.join("received"),
        ]
    }

    #[test]
    fn expect_init_scaffolds_the_full_tree() {
        let temp = tempfile::TempDir::new().unwrap();
        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();

        for path in expect_paths(&expect_dir) {
            assert!(path.exists(), "scaffold should create {}", path.display());
        }
        assert!(expect_dir.join("expectations").is_dir());
        assert!(expect_dir.join("goldens").is_dir());
        assert!(expect_dir.join("received").is_dir());
    }

    #[test]
    fn expect_init_gitignore_ignores_received_not_goldens() {
        let temp = tempfile::TempDir::new().unwrap();
        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();
        let gitignore = std::fs::read_to_string(expect_dir.join(GITIGNORE_FILE)).unwrap();

        assert!(
            gitignore.lines().any(|line| line.trim() == "received/"),
            "gitignore must ignore received/, got: {gitignore:?}"
        );
        assert!(
            !gitignore.lines().any(|line| line.trim() == "*"),
            "gitignore must not blanket-ignore with `*`, got: {gitignore:?}"
        );
        assert!(
            !gitignore.contains("goldens"),
            "gitignore must not ignore goldens/, got: {gitignore:?}"
        );
    }

    #[test]
    fn expect_init_is_idempotent_and_never_overwrites() {
        let temp = tempfile::TempDir::new().unwrap();
        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();

        // Simulate a user edit to two scaffolded files.
        let config_path = expect_dir.join(CONFIG_FILE);
        let example_path = expect_dir.join(EXAMPLE_FILE);
        std::fs::write(&config_path, "# user edited\n").unwrap();
        std::fs::write(&example_path, "# user edited example\n").unwrap();

        // Re-running must not clobber the edits, and must not error.
        scaffold_expect_dir(temp.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(&config_path).unwrap(),
            "# user edited\n",
            "config.toml must not be overwritten on re-run"
        );
        assert_eq!(
            std::fs::read_to_string(&example_path).unwrap(),
            "# user edited example\n",
            "example.expect.md must not be overwritten on re-run"
        );
    }

    #[test]
    fn expect_init_gitignore_reconcile_is_idempotent() {
        let temp = tempfile::TempDir::new().unwrap();
        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();
        let gitignore_path = expect_dir.join(GITIGNORE_FILE);
        let first = std::fs::read_to_string(&gitignore_path).unwrap();

        ensure_gitignore(&expect_dir).unwrap();
        let second = std::fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(
            first, second,
            "reconciling an up-to-date gitignore is a no-op"
        );
    }

    #[test]
    fn expect_init_detected_rust_project_yields_cli_surface_default() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();

        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();
        let config = std::fs::read_to_string(expect_dir.join(CONFIG_FILE)).unwrap();

        let expected = surface_name(Surface::Cli);
        assert!(
            config.contains(&expected),
            "a Rust project should record the `{expected}` surface default, got: {config}"
        );
    }

    #[test]
    fn expect_init_detected_http_project_yields_http_surface_default() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{\"name\":\"x\"}\n").unwrap();

        let expect_dir = scaffold_expect_dir(temp.path()).unwrap();
        let config = std::fs::read_to_string(expect_dir.join(CONFIG_FILE)).unwrap();

        let expected = surface_name(Surface::Http);
        assert!(
            config.contains(&expected),
            "a Node.js project should record the `{expected}` surface default, got: {config}"
        );
    }

    #[test]
    fn expect_init_no_project_falls_back_to_default_surface() {
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(detected_surfaces(temp.path()), vec![DEFAULT_SURFACE]);
    }

    #[test]
    fn expect_init_surface_mapping_is_exhaustive_and_sensible() {
        // Spot-check the table's intent across the three surface buckets.
        assert_eq!(surface_for_project_type(ProjectType::Rust), Surface::Cli);
        assert_eq!(surface_for_project_type(ProjectType::Go), Surface::Cli);
        assert_eq!(surface_for_project_type(ProjectType::NodeJs), Surface::Http);
        assert_eq!(
            surface_for_project_type(ProjectType::JavaMaven),
            Surface::Http
        );
        assert_eq!(surface_for_project_type(ProjectType::Flutter), Surface::Gui);
    }

    #[test]
    fn expect_init_config_template_round_trips_to_engine_defaults() {
        let parsed = ExpectConfig::parse(CONFIG_TEMPLATE).expect("template must be valid config");
        assert_eq!(
            parsed,
            ExpectConfig::default(),
            "the scaffolded config.toml must be the documented all-defaults config"
        );
    }

    #[test]
    fn expect_init_config_contents_still_parse_with_surface_header() {
        // The prepended detected-surface comment must not break TOML parsing.
        let contents = config_contents(&[Surface::Cli, Surface::Http]);
        let parsed = ExpectConfig::parse(&contents).expect("config with header must parse");
        assert_eq!(parsed, ExpectConfig::default());
        assert!(contents.contains("cli"));
        assert!(contents.contains("http"));
    }

    #[test]
    fn expect_init_metadata_and_scope_gate() {
        let tool = ExpectTool::new();
        assert_eq!(Initializable::name(&tool), "expect");
        assert_eq!(Initializable::display_name(&tool), "Expectations");
        assert_eq!(Initializable::category(&tool), "tools");
        assert_eq!(tool.priority(), 45);
        assert!(tool.is_applicable(&InitScope::Project));
        assert!(tool.is_applicable(&InitScope::Local));
        assert!(!tool.is_applicable(&InitScope::User));
    }

    #[test]
    fn expect_init_skips_user_scope_without_scaffolding() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        let tool = ExpectTool::new();
        let results = tool.init(&InitScope::User, &NullReporter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, InitStatus::Skipped);
    }
}
