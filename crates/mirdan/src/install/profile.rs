//! ── Profile installer (the single, data-driven init/deinit) ──────────────────
//!
//! A `Profile` is the *only* thing that differs between consumers (sah, the tool
//! CLIs, the kanban desktop app): a declarative manifest of what to install — an
//! MCP server, a selection of builtin skills, a selection of builtin agents, and
//! the sah-only statusline flag. `init_profile`/`deinit_profile` are
//! the single code path that interprets that data; there are no per-consumer
//! branches. Builtin skills and agents are rendered once through the prompt
//! library's Liquid engine (so `{% include "_partials/..." %}` references expand)
//! and deployed via the store + symlink mechanism — the same one
//! `deploy_skill_to_agents` / `deploy_agent_to_agents` already use.

use std::path::{Path, PathBuf};

use swissarmyhammer_agents::AgentResolver;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_skills::SkillResolver;
use swissarmyhammer_templating::TemplateLibrary;

use crate::agents::{self, AgentDef};
use crate::mcp_config::{self, string_newtype, McpServerEntry, ServersKey, ToolName};
use crate::package_type::PackageType;
use crate::registry::RegistryError;
use crate::settings;
use crate::store;
use crate::strategy::claude_settings::{
    permissions_deny_pointer, POINTER_KEY_DENY, POINTER_KEY_PERMISSIONS,
};

use super::applier::{
    for_each_detected_agent, register_mcp_server, scope_is_global, unregister_mcp_server,
    AgentAction, APPLIER_COMPONENT,
};
use super::deploy::{deploy_agent_to_agents_at, deploy_skill_to_agents_at};
use super::uninstall::{uninstall_agent_at, uninstall_skill_at};
use super::{copy_dir_recursive, rooted, temp_dir_error, temp_subdir_error};

// ── Parameter newtypes ───────────────────────────────────────────────────────
//
// Several helpers below carry two or three strings that mean entirely
// different things. Spelled `&str` they are interchangeable, so a caller can
// hand them over in the wrong order and the compiler stays silent. Each one
// gets its own type through the crate's `string_newtype!` macro — the same
// macro, and the same reason, as `ServersKey` and `ToolName`. An MCP server
// name is already `ToolName`, so these do not restate it.
//
// A helper that carries one of these strings alone takes the same type, so one
// concept keeps one spelling across the file. `InitResult::ok` takes the
// component name and the message as two adjacent strings, so every component
// name this file reports under crosses that boundary as a `ComponentName`.

string_newtype! {
    /// The store name of one builtin profile item, such as `commit`.
    ItemName
}

string_newtype! {
    /// The rendered body of one builtin item's manifest, staged as that file's
    /// contents.
    ManifestContent
}

string_newtype! {
    /// The manifest file name one builtin item is staged under, such as
    /// `SKILL.md`. Carries `ProfileItemKind::file_name` into the staging call.
    ManifestFilename
}

string_newtype! {
    /// The verb one reporter action is announced with, such as `Registered`.
    ActionVerb
}

string_newtype! {
    /// The preposition joining a reporter action to the agent it names, such
    /// as `for` in `sah MCP server for Claude Code`.
    Preposition
}

string_newtype! {
    /// The component name one step reports its [`InitResult`] under, such as
    /// `profile-skills`.
    ComponentName
}

string_newtype! {
    /// The label naming one family of builtin items in reporter text, such as
    /// `skill`. The deinit counterpart of `ProfileItemKind::label`.
    ItemKind
}

// ── Shared literals ──────────────────────────────────────────────────────────
//
// Each literal below is read at two or more sites that must agree, or is the
// sibling of such a literal in one expression. One constant is the single
// source of each one, so a change to the text reaches every site.
//
// The component names are `pub(crate)` because the tests that pin each
// reported row name the same constant the reporting code does, the way
// `APPLIER_COMPONENT` already is. A test that spells the name itself is a
// second source of that text.

/// The component name the skill step of a profile reports under.
pub(crate) const PROFILE_SKILLS_COMPONENT: &str = "profile-skills";

/// The component name the agent step of a profile reports under.
pub(crate) const PROFILE_AGENTS_COMPONENT: &str = "profile-agents";

/// The component name the validator step of a profile reports under.
pub(crate) const PROFILE_VALIDATORS_COMPONENT: &str = "profile-validators";

/// The reporter label for one builtin skill.
const SKILL_ITEM_LABEL: &str = "skill";

/// The reporter label for one builtin agent.
const AGENT_ITEM_LABEL: &str = "agent";

/// The file name of the discovery README at each builtin store root. The
/// install writes this file and the deinit removes it, so the two must agree.
const STORE_README_FILE_NAME: &str = "README.md";

/// The name prefix the validator loader reserves for a directory of shared
/// partials. Such a directory is never a validator set, however it is spelled
/// inside, so [`validator_set_names_in`] passes over it exactly as the loader
/// does.
const PARTIALS_DIR_NAME_PREFIX: &str = "_";

/// The reporter verb for content this installer wrote.
const VERB_DEPLOYED: &str = "Deployed";

/// The reporter verb for content this installer removed.
const VERB_REMOVED: &str = "Removed";

/// The reporter verb for a settings fragment this installer merged.
const VERB_INSTALLED: &str = "Installed";

/// The reporter verb for an MCP server this installer registered.
const VERB_REGISTERED: &str = "Registered";

/// Selects which builtin items (skills, agents, or validator sets) a profile
/// installs.
///
/// The same shape serves all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Every builtin item.
    All,
    /// The named items, in the given order. Unknown names are skipped.
    Named(Vec<String>),
    /// A single named item.
    Single(String),
}

impl Selector {
    /// Resolve this selector against the set of `available` builtin names,
    /// returning the selected names (sorted for `All`, source-ordered for
    /// `Named`/`Single`).
    pub(crate) fn select(&self, available: &std::collections::HashSet<String>) -> Vec<String> {
        match self {
            Selector::All => {
                let mut names: Vec<String> = available.iter().cloned().collect();
                names.sort();
                names
            }
            Selector::Named(names) => names
                .iter()
                .filter(|n| available.contains(*n))
                .cloned()
                .collect(),
            Selector::Single(name) => {
                if available.contains(name) {
                    vec![name.clone()]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

/// The MCP server a profile registers across detected agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMcpServer {
    /// The server name (the key under which it is registered, e.g. `sah`).
    pub name: String,
    /// The command to launch the server (e.g. `sah`).
    pub command: String,
    /// Arguments passed to the command (e.g. `["serve"]`).
    pub args: Vec<String>,
}

impl ProfileMcpServer {
    /// A server whose binary name *is* the launch command, started with the
    /// single `serve` argument: `{ name, command: name, args: ["serve"] }`.
    ///
    /// This is the shape every tool CLI and sah register — the binary registers
    /// itself under its own name and runs `<name> serve` — so they declare it
    /// with one value instead of repeating the verbatim triple.
    pub fn serve(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            command: name.clone(),
            name,
            args: vec!["serve".to_string()],
        }
    }
}

/// A declarative manifest of what a CLI or app installs.
///
/// This is the single value that differs per consumer; [`init_profile`] /
/// [`deinit_profile`] interpret it with no per-consumer branching. The sah-only
/// concern (`statusline`) is a plain declarative flag so that sah
/// is "just a bigger profile" rather than a special case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// The MCP server to register across detected agents, if any.
    pub mcp_server: Option<ProfileMcpServer>,
    /// Which builtin skills to render and deploy, if any.
    pub skills: Option<Selector>,
    /// Which builtin agents to render and deploy, if any.
    pub agents: Option<Selector>,
    /// Which builtin validator sets to materialize onto disk, if any.
    ///
    /// Like tools (which deploy store-only, no symlink), validators are
    /// materialized through the shared mirdan store into the validators store
    /// directory (`~/.validators/` global or `./.validators/` project) as a
    /// read-only reference copy that the loader reads at lowest precedence.
    pub validators: Option<Selector>,
    /// Whether to install the Claude Code statusline (`sah statusline`).
    pub statusline: bool,
    /// Whether to install the edit-surface redirect fragment: a
    /// `permissions.deny` on the native `Edit`/`Write` tools so every
    /// mutation flows through the served `files` MCP replacement (the
    /// diagnostics-instrumented path), mirroring how `shell` supersedes `Bash`.
    /// The deny alone closes the surface — no `PreToolUse` hook. Claude-shaped
    /// and inert on hosts that don't read that key (see
    /// [`apply_profile_edit_redirect`]).
    pub edit_redirect: bool,
}

impl Profile {
    /// The shape every tool CLI installs: its own `<name> serve` MCP server
    /// plus a skill selection, and nothing else.
    ///
    /// Companion to [`ProfileMcpServer::serve`]. kanban, code-context, and
    /// shelltool differ only in the server name and which skills they deploy,
    /// so they declare that pair rather than respelling the whole struct and
    /// drifting apart. sah is not a tool profile — it also carries agents,
    /// validators, the statusline, and the edit redirect — so it builds its
    /// own value.
    pub fn tool(server_name: impl Into<String>, skills: Selector) -> Self {
        Self {
            mcp_server: Some(ProfileMcpServer::serve(server_name)),
            skills: Some(skills),
            ..Default::default()
        }
    }
}

/// Build the template context used to render builtin skill/agent bodies.
///
/// Exposes `{{version}}` (this crate's package version) so skill/agent metadata
/// and instructions that reference it expand. `TemplateLibrary::render_text`
/// additionally injects default variables and environment variables.
fn profile_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

/// Render a builtin skill's instructions and metadata through the prompt
/// library's Liquid engine, expanding `{% include "_partials/..." %}` and
/// `{{version}}`.
///
/// Falls back to the raw instructions when rendering fails (logging a warning),
/// matching the per-CLI deploy behavior so a partial-resolution failure degrades
/// instead of aborting the whole install.
fn render_profile_skill(
    library: &TemplateLibrary,
    ctx: &TemplateContext,
    skill: &swissarmyhammer_skills::Skill,
) -> (String, std::collections::HashMap<String, String>) {
    // Expose the skill's `agent` frontmatter as a template variable so shared
    // partials (e.g. `_partials/delegate-to-subagent`) can render `{{ agent }}`
    // without each skill hard-coding its delegate name. Cloned per skill because
    // `ctx` is shared across the whole profile.
    let mut skill_ctx = ctx.clone();
    if let Some(agent) = skill.agent.as_deref() {
        skill_ctx.set("agent".to_string(), serde_json::json!(agent));
    }

    let instructions = library
        .render_text(&skill.instructions, &skill_ctx)
        .unwrap_or_else(|err| {
            tracing::warn!(
                skill = skill.name.as_str(),
                error = %err,
                "skill template rendering failed, falling back to raw instructions"
            );
            skill.instructions.clone()
        });

    let mut metadata = skill.metadata.clone();
    render_metadata_values(&mut metadata, library, &skill_ctx);

    (instructions, metadata)
}

/// Render Liquid expressions in metadata values in place.
///
/// A value without template markers, or one that fails to render, is left
/// unchanged.
fn render_metadata_values(
    metadata: &mut std::collections::HashMap<String, String>,
    library: &TemplateLibrary,
    ctx: &TemplateContext,
) {
    for value in metadata.values_mut() {
        if value.contains("{{") || value.contains("{%") {
            if let Ok(rendered) = library.render_text(value, ctx) {
                *value = rendered;
            }
        }
    }
}

/// Write a builtin `README.md` at a store root to aid discovery, logging a
/// warning on failure rather than failing the install.
///
/// `content` is the co-located `builtin/<kind>/README.md` for the store, embedded
/// at the call site with `include_str!`. The README sits *beside* the item
/// subdirectories; the resolvers only consider subdirectories that contain a
/// manifest (`SKILL.md` / `AGENT.md` / `VALIDATOR.md`), so a top-level
/// `README.md` is ignored and never mistaken for an installable item. A write
/// failure must never abort an otherwise successful deploy. Call only after the
/// store has been populated so the README never creates an empty store dir.
fn write_store_readme_for(store_root: &Path, content: &str, reporter: &dyn InitReporter) {
    let result = std::fs::create_dir_all(store_root)
        .and_then(|()| std::fs::write(store_root.join(STORE_README_FILE_NAME), content));
    if let Err(e) = result {
        reporter.emit(&InitEvent::Warning {
            message: format!(
                "failed to write {}/{STORE_README_FILE_NAME}: {e}",
                store_root.display()
            ),
        });
    }
}

/// Remove the builtin discovery `README.md` from a store root on deinit, then
/// remove the now-empty store directory.
///
/// The README is builtin-owned, so deinit removes it the same way it removes the
/// builtin items. The store directory itself is removed only when nothing else
/// remains, so any user-authored items keep the directory (and are untouched).
fn prune_store_readme(store_root: &Path, reporter: &dyn InitReporter) {
    match std::fs::remove_file(store_root.join(STORE_README_FILE_NAME)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => reporter.emit(&InitEvent::Warning {
            message: format!(
                "failed to remove {}/{STORE_README_FILE_NAME}: {e}",
                store_root.display()
            ),
        }),
    }
    if let Ok(mut entries) = std::fs::read_dir(store_root) {
        if entries.next().is_none() {
            let _ = std::fs::remove_dir(store_root);
        }
    }
}

/// The per-kind constants of one builtin profile item family (skills or
/// agents): the manifest file name, the store selection, the store's
/// discovery README, and the reporter label.
struct ProfileItemKind {
    /// Manifest file name staged next to the resources (`SKILL.md` / `AGENT.md`).
    file_name: &'static str,
    /// `true` deploys through the skill store/dirs, `false` through the agent ones.
    is_skill: bool,
    /// Store root for the kind, by `global` flag.
    store_dir: fn(bool) -> PathBuf,
    /// Discovery README content written at the store root.
    readme: &'static str,
    /// Reporter label for one item (`"skill"` / `"agent"`).
    label: &'static str,
}

/// Resolve, render, and deploy the profile's selected builtin skills.
///
/// Returns the deduplicated list of agent targets the skills were deployed to.
fn install_profile_skills(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, RegistryError> {
    install_profile_items(
        selector,
        scope,
        root,
        reporter,
        &SkillResolver::new().resolve_builtins(),
        ProfileItemKind {
            file_name: "SKILL.md",
            is_skill: true,
            store_dir: store::skill_store_dir,
            readme: include_str!("../../../../builtin/skills/README.md"),
            label: SKILL_ITEM_LABEL,
        },
        |library, ctx, _name, skill| {
            let (instructions, metadata) = render_profile_skill(library, ctx, skill);
            swissarmyhammer_skills::deploy::format_skill_md(skill, &instructions, &metadata)
        },
        |skill| Some(&skill.resources.files),
    )
}

/// Resolve, render, and deploy the profile's selected builtin agents.
///
/// Returns the deduplicated list of agent targets the agents were deployed to.
fn install_profile_agents(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, RegistryError> {
    install_profile_items(
        selector,
        scope,
        root,
        reporter,
        &AgentResolver::new().resolve_builtins(),
        ProfileItemKind {
            file_name: "AGENT.md",
            is_skill: false,
            store_dir: store::agent_store_dir,
            readme: include_str!("../../../../builtin/agents/README.md"),
            label: AGENT_ITEM_LABEL,
        },
        render_profile_agent,
        |_agent| None,
    )
}

/// Render one builtin agent's body and metadata to its `AGENT.md` content.
///
/// The agent counterpart of [`render_profile_skill`]: the body is rendered
/// through the Liquid library (falling back to the raw instructions on error)
/// and metadata values holding template markers are rendered in place.
fn render_profile_agent(
    library: &TemplateLibrary,
    ctx: &TemplateContext,
    name: &str,
    agent: &swissarmyhammer_agents::Agent,
) -> String {
    let rendered_body = library
        .render_text(&agent.instructions, ctx)
        .unwrap_or_else(|err| {
            tracing::warn!(
                agent = name,
                error = %err,
                "agent template rendering failed, falling back to raw instructions"
            );
            agent.instructions.clone()
        });
    let mut rendered_agent = agent.clone();
    render_metadata_values(&mut rendered_agent.metadata, library, ctx);
    rendered_agent.to_agent_md(&rendered_body)
}

/// Resolve, render, and deploy one family of builtin profile items through
/// the shared store + symlink mechanism.
///
/// The single installer behind [`install_profile_skills`] and
/// [`install_profile_agents`]: the two differ only in the [`ProfileItemKind`]
/// constants, the `render` step that produces the staged manifest content,
/// and the `extra_files` the manifest links to (`None` for kinds without
/// progressive-disclosure resources).
#[allow(clippy::too_many_arguments)]
fn install_profile_items<T>(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
    builtins: &std::collections::HashMap<String, T>,
    kind: ProfileItemKind,
    render: impl Fn(&TemplateLibrary, &TemplateContext, &str, &T) -> String,
    extra_files: impl for<'a> Fn(&'a T) -> Option<&'a std::collections::HashMap<String, String>>,
) -> Result<Vec<String>, RegistryError> {
    let library = TemplateLibrary::default();
    let ctx = profile_template_context();

    let selected = selected_names(selector, builtins.keys().cloned());
    let count = selected.len();
    let empty_files = std::collections::HashMap::new();
    let mut targets: Vec<String> = Vec::new();
    for name in selected {
        let item = &builtins[&name];
        let content = render(&library, &ctx, &name, item);
        let deployed = stage_and_deploy_rendered(
            ItemName::from(name),
            ManifestContent::from(content),
            ManifestFilename::from(kind.file_name),
            extra_files(item).unwrap_or(&empty_files),
            scope,
            root,
            kind.is_skill,
        )?;
        merge_targets(&mut targets, deployed);
    }

    // Drop a discovery README at the store root so a user browsing the store
    // learns what it is and how to customize it. Only when something was
    // deployed (the store dir now exists); a deploy of >0 items cannot reach
    // here with an absent store.
    if count > 0 {
        let global = scope_is_global(scope);
        let store_root = rooted(root, global, (kind.store_dir)(global));
        write_store_readme_for(&store_root, kind.readme, reporter);
    }

    if !targets.is_empty() {
        reporter.emit(&InitEvent::Action {
            verb: VERB_DEPLOYED.to_string(),
            message: format!("{} {}(s) to {}", count, kind.label, targets.join(", ")),
        });
    }
    Ok(targets)
}

/// Materialize the profile's selected builtin validator sets onto disk through
/// the shared mirdan store mechanism.
///
/// Builtin validators are embedded in the binary (see
/// [`crate::builtin_validators`]). Each selected set is staged into a temp
/// directory as a real tree and copied into the validators store
/// (`~/.validators/<set>/` global or `./.validators/<set>/` project) with the
/// shared [`copy_dir_recursive`], rooted at `root` for project scope so the
/// operation never reads `current_dir()`. This is the same store-only path
/// [`deploy_tool`] uses (no symlink — the validator loader reads the store
/// directly).
///
/// # Reference-copy policy
///
/// Builtin-owned files are OWNED by the installer: every embedded file is
/// copied (overwritten) on each install so builtin updates always propagate,
/// exactly like the generated `.skills/` reference copy. Crucially the copy is
/// additive at the destination — [`copy_dir_recursive`] writes the embedded
/// files but never deletes the set directory wholesale — so a user-authored
/// validator added inside a builtin set dir, and any entirely user-created set,
/// survive untouched. To customize a builtin, a user creates a NEW validator
/// that wins by loader precedence rather than editing the deployed reference in
/// place.
///
/// Returns the set names that were materialized.
fn install_profile_validators(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, RegistryError> {
    let sets = crate::builtin_validators::builtin_validators_by_set();

    let global = scope_is_global(scope);
    let target_root = rooted(root, global, store::validators_store_dir(global));

    let selected = selected_names(selector, sets.keys().map(|name| name.to_string()));
    let mut materialized: Vec<String> = Vec::new();
    for set in &selected {
        let files = &sets[set.as_str()];

        // Stage the set's embedded files into a temp dir as a real tree (set
        // prefix stripped so the temp holds the set's *contents*), then copy
        // that tree into the store via the shared helper — the same store-only
        // deploy `deploy_tool` performs.
        let staged = stage_validator_set(set, files)?;
        let dest_set_dir = target_root.join(set);
        copy_dir_recursive(staged.path(), &dest_set_dir)?;

        materialized.push(set.clone());
    }

    // Drop a discovery README at the validator store root (see the skills path).
    if !materialized.is_empty() {
        write_store_readme_for(
            &target_root,
            include_str!("../../../../builtin/validators/README.md"),
            reporter,
        );
    }

    if !materialized.is_empty() {
        reporter.emit(&InitEvent::Action {
            verb: VERB_DEPLOYED.to_string(),
            message: format!(
                "{} validator set(s) to {}",
                materialized.len(),
                target_root.display()
            ),
        });
    }
    Ok(materialized)
}

/// Stage one builtin validator set's embedded files into a temp directory as a
/// real directory tree, returning the temp dir whose root holds the set's
/// contents (`VALIDATOR.md`, `rules/*.md`, …) with the leading `<set>/` prefix
/// stripped.
///
/// The returned [`tempfile::TempDir`] must be kept alive until the copy into the
/// store completes; dropping it removes the staged tree.
fn stage_validator_set(
    set: &str,
    files: &[(&'static str, &'static str)],
) -> Result<tempfile::TempDir, RegistryError> {
    let staged = tempfile::tempdir().map_err(temp_dir_error)?;
    let set_prefix = format!("{set}/");
    for (embedded_name, content) in files {
        // Embedded names are set-prefixed (e.g. `dead-code/rules/x.md`); strip
        // the `<set>/` prefix so the staged tree is the set's *contents*.
        let relative = embedded_name
            .strip_prefix(&set_prefix)
            .unwrap_or(embedded_name);
        let dest = staged.path().join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RegistryError::Validation(format!("failed to create {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&dest, content).map_err(|e| {
            RegistryError::Validation(format!("failed to write {}: {e}", dest.display()))
        })?;
    }
    Ok(staged)
}

/// Remove the validator sets the selector resolves to from the STORE,
/// mirroring [`install_profile_validators`].
///
/// # Why this reads the store and not the embed
///
/// The embedded roster names the sets THIS binary installs. A deinit that
/// walked that roster removed only those sets, so a set an older binary wrote
/// — a name the current roster no longer holds — was named by no embedded path
/// and stayed in the store forever. The user then kept running every rule that
/// set carries. Nothing on disk marks which binary wrote a set, and there is no
/// per-install record to consult, so "is this directory a validator set" is the
/// whole of what deinit can measure. [`validator_set_names_in`] makes that
/// measurement.
///
/// # What happens to a file a user wrote
///
/// A set is removed whole, so a rule a user added inside a builtin set, and a
/// set a user wrote by hand, both go with the set. The store serves the review
/// engine alone, and deinit removes the review engine, so the validators it
/// served go too. Store content that is NOT a set stays: a loose file at the
/// store root, and a directory that carries no manifest. Each survivor keeps
/// the store directory itself alive, because [`prune_store_readme`] removes
/// that directory only when nothing is left in it.
///
/// Returns the names of the sets that were removed. A set that fails to remove
/// is reported as a Warning through `reporter` and is not counted.
fn deinit_profile_validators(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<String> {
    let global = scope_is_global(scope);
    let target_root = rooted(root, global, store::validators_store_dir(global));

    let selected = selected_names(selector, validator_set_names_in(&target_root));
    let mut removed: Vec<String> = Vec::new();
    for set in selected {
        let set_dir = target_root.join(&set);
        match std::fs::remove_dir_all(&set_dir) {
            Ok(()) => removed.push(set),
            Err(e) => reporter.emit(&InitEvent::Warning {
                message: format!("failed to remove {}: {e}", set_dir.display()),
            }),
        }
    }
    removed
}

/// The name of every validator set the store rooted at `store_root` holds.
///
/// A set is a subdirectory that carries a [`PackageType::Validator`] manifest
/// and whose name does not open with [`PARTIALS_DIR_NAME_PREFIX`]. That is the
/// rule the validator loader reads the store by, so deinit and the loader agree
/// on what a set is. A store directory that is not there holds no set.
fn validator_set_names_in(store_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(store_root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with(PARTIALS_DIR_NAME_PREFIX) {
            continue;
        }
        if entry
            .path()
            .join(PackageType::Validator.manifest_file())
            .is_file()
        {
            names.push(name);
        }
    }
    names
}

/// Stage `content` as `<tmp>/<name>/<file_name>` and deploy it via the store +
/// symlink mechanism, rooting at `root` when supplied so deployment never reads
/// `current_dir()`.
///
/// `extra_files` are progressive-disclosure resources (e.g.
/// `references/RUST_COVERAGE.md`) staged beside the manifest under their
/// relative paths — the rendered body links to them relatively, so a store
/// entry without them is broken.
///
/// `is_skill` selects the skill store/dirs vs. the agent store/dirs.
fn stage_and_deploy_rendered(
    name: ItemName,
    content: ManifestContent,
    file_name: ManifestFilename,
    extra_files: &std::collections::HashMap<String, String>,
    scope: InitScope,
    root: Option<&Path>,
    is_skill: bool,
) -> Result<Vec<String>, RegistryError> {
    if !store::is_safe_name(name.as_str()) {
        return Err(RegistryError::Validation(format!(
            "unsafe name: {:?}",
            name.as_str()
        )));
    }
    let temp_dir = tempfile::tempdir().map_err(temp_dir_error)?;
    let item_dir = temp_dir.path().join(name.as_str());
    std::fs::create_dir_all(&item_dir).map_err(|e| temp_subdir_error("item", e))?;
    std::fs::write(item_dir.join(file_name.as_str()), content.as_str())
        .map_err(|e| RegistryError::Validation(format!("failed to write {file_name}: {e}")))?;

    for (rel_path, file_content) in extra_files {
        if !store::is_safe_relative_path(rel_path) {
            return Err(RegistryError::Validation(format!(
                "unsafe resource path in {name}: {rel_path:?}"
            )));
        }
        let dest = item_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RegistryError::Validation(format!("failed to create resource dir: {e}"))
            })?;
        }
        std::fs::write(&dest, file_content).map_err(|e| {
            RegistryError::Validation(format!("failed to write resource {rel_path}: {e}"))
        })?;
    }

    let global = scope_is_global(scope);
    if is_skill {
        deploy_skill_to_agents_at(name.as_str(), &item_dir, None, global, root)
    } else {
        deploy_agent_to_agents_at(name.as_str(), &item_dir, None, global, root)
    }
}

/// Append `new` targets to `targets`, skipping duplicates (preserving order).
fn merge_targets(targets: &mut Vec<String>, new: Vec<String>) {
    for target in new {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
}

/// Top-level key for Claude Code's statusline configuration block.
const STATUSLINE_KEY: &str = "statusLine";

/// The statusline configuration value a profile installs.
///
/// Mirrors the Claude-conventional `{type: "command", command: "sah statusline"}`
/// block the CLI previously wrote by hand, so the `statusline` flag installs
/// exactly the same configuration through the data-driven profile path.
pub(crate) fn desired_statusline_value() -> serde_json::Value {
    serde_json::json!({
        "type": "command",
        "command": "sah statusline"
    })
}

/// Resolve a detected agent's settings/instructions file for `scope`, rooting
/// project-scope relative paths against `root` so the operation is CWD-free.
///
/// `global_resolve` returns the agent's absolute global path (user scope);
/// `project_resolve` returns the agent's project-relative path (project/local
/// scope), which is joined onto `root` via [`rooted`]. Returns `None` when the
/// agent declares no path for the scope.
fn resolve_agent_file(
    agent: &AgentDef,
    global: bool,
    root: Option<&Path>,
    global_resolve: impl Fn(&AgentDef) -> Option<PathBuf>,
    project_resolve: impl Fn(&AgentDef) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if global {
        global_resolve(agent)
    } else {
        project_resolve(agent).map(|relative| rooted(root, global, relative))
    }
}

/// One installable settings fragment: its component name, its reporter
/// subject, and the per-file apply function.
struct SettingsFragment {
    /// `InitResult` component name (e.g. `"profile-statusline"`).
    component: &'static str,
    /// Reporter message subject (e.g. `"statusline"`).
    subject: &'static str,
    /// Merge (install) or strip (remove) the fragment in one settings file.
    apply_at: fn(&Path, bool) -> Result<bool, RegistryError>,
}

/// The statusline settings fragment.
const STATUSLINE_FRAGMENT: SettingsFragment = SettingsFragment {
    component: "profile-statusline",
    subject: "statusline",
    apply_at: apply_statusline_at,
};

/// The edit-surface redirect settings fragment.
const EDIT_REDIRECT_FRAGMENT: SettingsFragment = SettingsFragment {
    component: "profile-edit-redirect",
    subject: "edit-surface redirect",
    apply_at: apply_edit_redirect_at,
};

/// Merge one settings fragment into every detected agent's settings file, or
/// strip it when `install` is false. Root-aware so project-scope settings
/// files resolve against `root` instead of `current_dir()`.
///
/// The single driver behind the statusline and edit-redirect steps: the two
/// differ only in their [`SettingsFragment`] constants.
fn apply_profile_settings_fragment(
    install: bool,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
    fragment: &SettingsFragment,
) -> Vec<InitResult> {
    let component = ComponentName::new(fragment.component);
    let subject = fragment.subject;
    let apply_at = fragment.apply_at;
    let verb = if install {
        VERB_INSTALLED
    } else {
        VERB_REMOVED
    };
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some(path) = resolve_agent_file(
                agent,
                global,
                root,
                agents::agent_global_settings_file,
                agents::agent_project_settings_file,
            ) else {
                return Ok(None);
            };
            Ok(apply_at(&path, install)?.then(|| AgentAction {
                verb: verb.to_string(),
                message: format!("{subject} for {} ({})", agent.name, path.display()),
            }))
        },
        |changed| {
            InitResult::ok(
                component.as_str(),
                format!("{verb} {subject} for {changed} agent(s)"),
            )
        },
    )
}

/// Set or remove the `statusLine` block in the settings file at `path`.
///
/// On install, missing files are created. On removal, a missing file is a no-op.
/// Returns `Ok(true)` when the file changed, `Ok(false)` when already in the
/// desired state.
pub(crate) fn apply_statusline_at(path: &Path, install: bool) -> Result<bool, RegistryError> {
    if !install && !path.exists() {
        return Ok(false);
    }
    let mut settings = settings::read_json(path)?;
    let changed = if install {
        settings::set_object(&mut settings, STATUSLINE_KEY, desired_statusline_value())
    } else {
        settings::remove_key(&mut settings, STATUSLINE_KEY)
    };
    if changed {
        settings::write_json(path, &settings)?;
    }
    Ok(changed)
}

// ── Edit-surface redirect (close the superseded-native surface) ──────
//
// An MCP server cannot disable a host's *native* built-in tools, so closing
// the surface sah supersedes — forcing every shell command through the served
// `shell` tool and every file operation through the served `files` tool, so
// diagnostics always ride the result — ships as an installable Claude Code
// settings fragment that `permissions.deny`s those natives. The deny alone
// closes the surface: the model is steered to the served replacements, and no
// redirect hook is needed. (An earlier version also installed a `PreToolUse`
// redirect hook; it was redundant once `files` is served as the replacement,
// and it dead-ended wherever that replacement isn't mounted — so it was
// dropped.)
//
// The deny is a plain Claude-shaped settings key, harmless on agents that don't
// read it — so the fragment is a no-op there rather than an error.

/// The native Claude Code tools sah supersedes, denied by the redirect fragment.
///
/// The deny forces `Bash` to the served `shell` tool and `Read`/`Edit`/`Write`
/// to the served `files` tool. Kept as data (not spelled into control flow) so
/// the installer that writes the deny and the
/// [`permissions_present`](crate::status::permissions_present) detector that
/// reads it back derive from one source and cannot drift.
pub const SUPERSEDED_NATIVE_DENY_TOOLS: &[&str] = &["Bash", "Edit", "Read", "Write"];

/// The installable Claude Code settings fragment that closes the superseded
/// surface.
///
/// Denies the natives in [`SUPERSEDED_NATIVE_DENY_TOOLS`] — and nothing else.
/// The model is steered to the served `shell` and `files` MCP replacements. The
/// deny alone closes the surface; a `PreToolUse` redirect would be redundant and
/// breaks wherever the replacement isn't mounted (the host's own write attempts,
/// nested contexts). This is the single source of truth for the fragment's
/// shape; [`apply_edit_redirect_at`] merges its parts into a real settings file
/// without clobbering unrelated keys.
pub(crate) fn desired_edit_redirect_fragment() -> serde_json::Value {
    serde_json::json!({
        POINTER_KEY_PERMISSIONS: {
            POINTER_KEY_DENY: SUPERSEDED_NATIVE_DENY_TOOLS,
        }
    })
}

/// Merge the edit-redirect fragment into the settings file at `path`, or strip
/// it when `install` is false.
///
/// On install, every deny entry of [`SUPERSEDED_NATIVE_DENY_TOOLS`] is added
/// idempotently (existing unrelated deny entries are preserved, and re-running
/// adds nothing). On removal, exactly those entries are taken back out and
/// unrelated keys are left intact. A missing
/// file is created on install and a no-op on removal. Returns `Ok(true)` when the
/// file changed.
pub(crate) fn apply_edit_redirect_at(path: &Path, install: bool) -> Result<bool, RegistryError> {
    if !install && !path.exists() {
        return Ok(false);
    }

    // Derive the exact pieces to merge from the canonical fragment, so the
    // fragment validated by tests is the same shape written to disk — there is
    // one source of truth for what "the edit-redirect" is.
    let fragment = desired_edit_redirect_fragment();
    let deny_entries = fragment[POINTER_KEY_PERMISSIONS][POINTER_KEY_DENY]
        .as_array()
        .unwrap_or_else(|| {
            panic!("fragment {POINTER_KEY_PERMISSIONS}.{POINTER_KEY_DENY} is an array")
        });

    let pointer = permissions_deny_pointer();
    let mut settings = settings::read_json(path)?;
    let mut changed = false;
    for entry in deny_entries {
        let one = if install {
            settings::ensure_array_contains(&mut settings, &pointer, entry)
        } else {
            settings::remove_from_array(&mut settings, &pointer, entry)
        };
        changed |= one;
    }

    if changed {
        settings::write_json(path, &settings)?;
    }
    Ok(changed)
}

/// Install the edit-redirect fragment into every detected agent's settings file,
/// or strip it when `install` is false. Root-aware so project-scope settings
/// files resolve against `root` instead of `current_dir()`. Mirrors
/// [`apply_profile_statusline`]: the fragment is Claude-shaped and inert on
/// agents that do not read those keys.
/// Register the profile's MCP server across detected agents.
///
/// With no explicit `root`, dispatches through the strategy-aware
/// [`register_mcp_server`] applier (handling Claude local scope, generic JSON
/// agents, etc.). With an explicit `root`, registers directly into each agent's
/// root-relative MCP config so the operation never reads `current_dir()`.
fn install_profile_mcp(
    server: &ProfileMcpServer,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let entry = McpServerEntry {
        command: server.command.clone(),
        args: server.args.clone(),
        env: std::collections::BTreeMap::new(),
    };
    match root {
        None => register_mcp_server(scope, &server.name, &entry, reporter),
        Some(root) => {
            register_mcp_server_at(root, &ToolName::new(&server.name), &entry, scope, reporter)
        }
    }
}

/// Resolve a detected agent's MCP config and its root-relative config-file path.
///
/// Returns the agent's [`McpConfigDef`] paired with the file to write: the
/// absolute global config under `User` scope (`global`), or the project config
/// joined onto `root` otherwise. `None` when the agent declares no MCP config or
/// no config path for the scope, signalling the caller to skip it.
fn resolve_agent_mcp_config<'a>(
    agent: &'a AgentDef,
    global: bool,
    root: &Path,
) -> Option<(&'a agents::McpConfigDef, PathBuf)> {
    let mcp_cfg = agent.mcp_config.as_ref()?;
    let config_path = if global {
        agents::agent_global_mcp_config(agent)
    } else {
        agents::agent_project_mcp_config(agent).map(|p| root.join(p))
    }?;
    Some((mcp_cfg, config_path))
}

/// Root-explicit MCP registration: write the server entry into each detected
/// agent's project MCP config resolved against `root`.
///
/// Only project/local scope is rooted; user scope uses the agent's absolute
/// global MCP config. Agents without an MCP config for the scope are skipped.
fn register_mcp_server_at(
    root: &Path,
    server_name: &ToolName,
    entry: &McpServerEntry,
    scope: InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    apply_mcp_operation_at(
        root,
        server_name,
        scope,
        reporter,
        ActionVerb::new(VERB_REGISTERED),
        Preposition::new("for"),
        |mcp_cfg, config_path| {
            mcp_config::register_mcp_server(
                config_path,
                &ServersKey::new(&mcp_cfg.servers_key),
                server_name,
                entry,
                &mcp_cfg.entry_extras,
            )?;
            Ok(true)
        },
    )
}

/// Root-explicit MCP unregistration mirroring [`register_mcp_server_at`].
fn unregister_mcp_server_at(
    root: &Path,
    server_name: &ToolName,
    scope: InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    apply_mcp_operation_at(
        root,
        server_name,
        scope,
        reporter,
        ActionVerb::new(VERB_REMOVED),
        Preposition::new("from"),
        |mcp_cfg, config_path| {
            mcp_config::unregister_mcp_server(
                config_path,
                &ServersKey::new(&mcp_cfg.servers_key),
                server_name,
            )
        },
    )
}

/// Drive one root-explicit MCP config operation across detected agents.
///
/// The shared skeleton of [`register_mcp_server_at`] and
/// [`unregister_mcp_server_at`]: resolve each agent's MCP config, run
/// `operation`, and report `verb`/`preposition` for each agent that changed.
#[allow(clippy::too_many_arguments)]
fn apply_mcp_operation_at(
    root: &Path,
    server_name: &ToolName,
    scope: InitScope,
    reporter: &dyn InitReporter,
    verb: ActionVerb,
    preposition: Preposition,
    operation: impl Fn(&agents::McpConfigDef, &Path) -> Result<bool, RegistryError>,
) -> Vec<InitResult> {
    let component = ComponentName::new(APPLIER_COMPONENT);
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some((mcp_cfg, config_path)) = resolve_agent_mcp_config(agent, global, root) else {
                return Ok(None);
            };
            Ok(operation(mcp_cfg, &config_path)?.then(|| AgentAction {
                verb: verb.as_str().to_string(),
                message: format!("{server_name} MCP server {preposition} {}", agent.name),
            }))
        },
        |changed| {
            InitResult::ok(
                component.as_str(),
                format!("{verb} applied to {changed} agent(s)"),
            )
        },
    )
}

/// Install everything a [`Profile`] declares, in priority order.
///
/// Steps, each a no-op when the profile does not declare it:
/// 1. register the `mcp_server` across detected agents (strategy-aware, or
///    root-explicit when `root` is `Some`),
/// 2. render the selected builtin skills with Liquid + the partial library and
///    deploy them via store + symlink,
/// 3. render and deploy the selected builtin agents the same way,
/// 4. apply the statusline when the profile sets that flag.
///
/// `root` makes the whole operation CWD-free: project-scope skill/agent stores,
/// agent directories, and MCP configs are resolved against `root` instead of the
/// process working directory. Pass `None` for the conventional CWD-rooted
/// behavior.
///
/// # Lockfile
///
/// Builtin skills/agents deployed here are deliberately *not* recorded in
/// `mirdan-lock.json`. That lockfile tracks registry-installed packages (download
/// URL + integrity hash) so they can be updated/reinstalled; builtins are shipped
/// in the binary and have no such identity. All builtin lifecycle — `mirdan
/// status`/`list` (filesystem scan of the skill/agent stores) and deinit (profile
/// selector) — is path- and profile-driven, never lockfile-driven, so the absent
/// entries are inert. This is an intentional simplification over the prior
/// per-component installers, which wrote inert `resolved: "builtin"` rows.
pub fn init_profile(
    profile: &Profile,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = Vec::new();

    if let Some(ref server) = profile.mcp_server {
        results.extend(install_profile_mcp(server, scope, root, reporter));
    }

    if let Some(ref selector) = profile.skills {
        run_install_step(
            &mut results,
            ComponentName::new(PROFILE_SKILLS_COMPONENT),
            install_profile_skills(selector, scope, root, reporter),
            |targets| {
                format!(
                    "{VERB_DEPLOYED} {SKILL_ITEM_LABEL}s to {}",
                    targets.join(", ")
                )
            },
        );
    }

    if let Some(ref selector) = profile.agents {
        run_install_step(
            &mut results,
            ComponentName::new(PROFILE_AGENTS_COMPONENT),
            install_profile_agents(selector, scope, root, reporter),
            |targets| {
                format!(
                    "{VERB_DEPLOYED} {AGENT_ITEM_LABEL}s to {}",
                    targets.join(", ")
                )
            },
        );
    }

    if let Some(ref selector) = profile.validators {
        run_install_step(
            &mut results,
            ComponentName::new(PROFILE_VALIDATORS_COMPONENT),
            install_profile_validators(selector, scope, root, reporter),
            |sets| format!("Materialized validator set(s): {}", sets.join(", ")),
        );
    }

    if profile.statusline {
        results.extend(apply_profile_settings_fragment(
            true,
            scope,
            root,
            reporter,
            &STATUSLINE_FRAGMENT,
        ));
    }

    if profile.edit_redirect {
        results.extend(apply_profile_settings_fragment(
            true,
            scope,
            root,
            reporter,
            &EDIT_REDIRECT_FRAGMENT,
        ));
    }

    results
}

/// Remove everything a [`Profile`] declares, mirroring [`init_profile`].
///
/// Unregisters the MCP server, removes the selected skills/agents from detected
/// agents, and strips the statusline block when the profile declared it. `root`
/// roots project-scope paths so deinit is CWD-free.
///
/// # Version coupling
///
/// The profile is the single source of truth, so deinit removes exactly the
/// builtin skills/agents the *current* binary's selector resolves to — not a
/// recorded record of what a past install wrote. If the builtin set drifts
/// between the version that installed and the version that deinits (a skill is
/// renamed or dropped), a now-absent name will not be swept, leaving an orphaned
/// symlink + store entry. This is inherent to the data-driven design (there is
/// no per-install manifest to consult); callers that must deinit across such a
/// drift should run deinit with the same binary version that installed.
///
/// Validators do not carry that drift. [`deinit_profile_validators`] reads the
/// STORE rather than the embedded roster, so a set an older binary wrote is
/// cleared with the rest. That doc states what the store is and what it costs a
/// user who wrote a set of their own.
pub fn deinit_profile(
    profile: &Profile,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = Vec::new();
    let global = scope_is_global(scope);

    if let Some(ref server) = profile.mcp_server {
        results.extend(deinit_profile_mcp(server, scope, root, reporter));
    }

    if let Some(ref selector) = profile.skills {
        run_deinit_step(
            &mut results,
            &resolved_skill_names(selector),
            ComponentName::new(PROFILE_SKILLS_COMPONENT),
            ItemKind::new(SKILL_ITEM_LABEL),
            store::skill_store_dir,
            |name| uninstall_skill_at(name, None, global, root),
            global,
            root,
            reporter,
        );
    }

    if let Some(ref selector) = profile.agents {
        run_deinit_step(
            &mut results,
            &resolved_agent_names(selector),
            ComponentName::new(PROFILE_AGENTS_COMPONENT),
            ItemKind::new(AGENT_ITEM_LABEL),
            store::agent_store_dir,
            |name| uninstall_agent_at(name, None, global, root).map(|_| ()),
            global,
            root,
            reporter,
        );
    }

    if let Some(ref selector) = profile.validators {
        let removed = deinit_profile_validators(selector, scope, root, reporter);
        if !removed.is_empty() {
            let component = ComponentName::new(PROFILE_VALIDATORS_COMPONENT);
            results.push(InitResult::ok(
                component.as_str(),
                format!("{VERB_REMOVED} {} validator set(s)", removed.len()),
            ));
        }
        prune_store_readme_at(store::validators_store_dir, global, root, reporter);
    }

    if profile.statusline {
        results.extend(apply_profile_settings_fragment(
            false,
            scope,
            root,
            reporter,
            &STATUSLINE_FRAGMENT,
        ));
    }

    if profile.edit_redirect {
        results.extend(apply_profile_settings_fragment(
            false,
            scope,
            root,
            reporter,
            &EDIT_REDIRECT_FRAGMENT,
        ));
    }

    results
}

/// Run one profile item installer and record its outcome on `results`.
///
/// Pushes an ok result built by `success_message` when anything deployed,
/// nothing on an empty deploy, and an error result naming `component` on
/// failure. The single outcome arm behind the skill, agent, and validator
/// steps of [`init_profile`].
fn run_install_step(
    results: &mut Vec<InitResult>,
    component: ComponentName,
    outcome: Result<Vec<String>, RegistryError>,
    success_message: impl Fn(&[String]) -> String,
) {
    match outcome {
        Ok(items) if !items.is_empty() => {
            results.push(InitResult::ok(component.as_str(), success_message(&items)))
        }
        Ok(_) => {}
        Err(e) => results.push(InitResult::error(component.as_str(), e.to_string())),
    }
}

/// Unregister the profile's MCP server, strategy-aware without `root` or
/// root-explicit with it. Mirrors [`install_profile_mcp`].
fn deinit_profile_mcp(
    server: &ProfileMcpServer,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    match root {
        None => unregister_mcp_server(scope, &server.name, reporter),
        Some(root) => unregister_mcp_server_at(root, &ToolName::new(&server.name), scope, reporter),
    }
}

/// Remove one family of builtin profile items and prune its store README.
///
/// The single removal arm behind the skill and agent steps of
/// [`deinit_profile`]: the two differ only in the resolved names, the
/// labels, the uninstall call, and the store directory.
#[allow(clippy::too_many_arguments)]
fn run_deinit_step(
    results: &mut Vec<InitResult>,
    names: &[String],
    component: ComponentName,
    kind: ItemKind,
    store_dir: fn(bool) -> PathBuf,
    uninstall_one: impl Fn(&str) -> Result<(), RegistryError>,
    global: bool,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) {
    results.extend(deinit_profile_items(
        names,
        component,
        kind,
        uninstall_one,
        reporter,
    ));
    // Remove the builtin discovery README and prune the store if now empty.
    prune_store_readme_at(store_dir, global, root, reporter);
}

/// Uninstall every named builtin item with `uninstall_one`, emitting a Warning
/// per failure, and return one ok result naming `component` when the selector
/// resolved any names.
///
/// The single removal loop behind the skill and agent arms of
/// [`deinit_profile`]: the two differ only in the uninstall call and labels.
fn deinit_profile_items(
    names: &[String],
    component: ComponentName,
    kind: ItemKind,
    uninstall_one: impl Fn(&str) -> Result<(), RegistryError>,
    reporter: &dyn InitReporter,
) -> Option<InitResult> {
    for name in names {
        if let Err(e) = uninstall_one(name) {
            reporter.emit(&InitEvent::Warning {
                message: format!("Failed to uninstall {name} {kind}: {e}"),
            });
        }
    }
    (!names.is_empty()).then(|| {
        InitResult::ok(
            component.as_str(),
            format!("{VERB_REMOVED} {} {kind}(s)", names.len()),
        )
    })
}

/// Prune the builtin discovery README (and the store dir when empty) at the
/// store root `store_dir` yields, rooted for project scope.
fn prune_store_readme_at(
    store_dir: fn(bool) -> PathBuf,
    global: bool,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) {
    prune_store_readme(&rooted(root, global, store_dir(global)), reporter);
}

/// Install a [`Profile`] and then run a registry of genuine tool-lifecycle
/// components, returning the combined results.
///
/// This is the single shared glue every profile consumer needs: the profile
/// installer ([`init_profile`]) followed by the tool's own `Initializable`
/// components (e.g. a `.code-context/` directory, `.kanban/` merge drivers, a
/// `Bash` denial). Each tool CLI and sah build their `Profile`, register their
/// tool-lifecycle components, and call this — instead of re-spelling the
/// "profile then registry, concatenate results" sequence in four places.
///
/// Results are ordered profile-first (matching every prior consumer): the MCP
/// server, skills, agents, statusline, then the registry components in
/// priority order. `root` is forwarded to [`init_profile`] for CWD-free
/// installs. Compute the exit code from the returned results' `status` fields.
pub fn init_profile_with_registry(
    profile: &Profile,
    registry: &InitRegistry,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    chain_results(
        init_profile(profile, scope, root, reporter),
        registry.run_all_init(&scope, reporter),
    )
}

/// Concatenate the results of two lifecycle steps in order.
///
/// The single result-merging step behind [`init_profile_with_registry`] and
/// [`deinit_profile_with_registry`].
fn chain_results(first: Vec<InitResult>, second: Vec<InitResult>) -> Vec<InitResult> {
    let mut results = first;
    results.extend(second);
    results
}

/// Deinstall a [`Profile`] after deinitializing a registry of tool-lifecycle
/// components, returning the combined results.
///
/// The mirror of [`init_profile_with_registry`]: tool-lifecycle teardown runs
/// first (reverse priority, via [`InitRegistry::run_all_deinit`]), then the
/// profile deinstaller ([`deinit_profile`]) unregisters the MCP server and
/// removes the selected skills/agents. This ordering matches every prior
/// consumer so a tool's directory is removed before its MCP registration.
pub fn deinit_profile_with_registry(
    profile: &Profile,
    registry: &InitRegistry,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    chain_results(
        registry.run_all_deinit(&scope, reporter),
        deinit_profile(profile, scope, root, reporter),
    )
}

/// Select the names a selector picks out of an iterator of available names.
fn selected_names(selector: &Selector, available: impl IntoIterator<Item = String>) -> Vec<String> {
    let available: std::collections::HashSet<String> = available.into_iter().collect();
    selector.select(&available)
}

/// Resolve the builtin skill names a selector picks (for deinit).
fn resolved_skill_names(selector: &Selector) -> Vec<String> {
    selected_names(
        selector,
        SkillResolver::new().resolve_builtins().into_keys(),
    )
}

/// Resolve the builtin agent names a selector picks (for deinit).
fn resolved_agent_names(selector: &Selector) -> Vec<String> {
    selected_names(
        selector,
        AgentResolver::new().resolve_builtins().into_keys(),
    )
}
