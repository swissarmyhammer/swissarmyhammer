//! Render mirdan [`Profile`]s into Claude Code plugin trees.
//!
//! A Claude Code plugin is a directory containing `.claude-plugin/plugin.json`
//! plus convention subdirectories (`skills/`, `agents/`, `commands/`) and an
//! `mcpServers` block. A mirdan [`Profile`] already declares everything a plugin
//! needs — an MCP server, a selection of builtin skills, a selection of builtin
//! agents — so packaging a plugin is the *same* data interpreted by a third
//! renderer, alongside `init_profile` (deploy into agent configs) and
//! `deinit_profile` (remove).
//!
//! The skill/agent bodies are rendered through the **identical** path the deploy
//! installer uses ([`crate::install::render_profile_skill`] /
//! [`crate::install::render_profile_agent`]), so a profile's `kanban` skill is
//! byte-for-byte the same whether it is symlinked into `.claude/skills/` by
//! `kanban init` or written into a packaged plugin here. There is no second
//! rendering path to drift.
//!
//! ## Binary distribution — strategy A (PATH)
//!
//! The plugin's `mcpServers` entry is written verbatim from the profile's
//! [`ProfileMcpServer`]: `{ command: "<name>", args: ["serve"] }`. This is the
//! **PATH strategy** — the plugin bundles the (platform-independent) rendered
//! content, and the native `serve` binary is expected on `PATH`, installed via
//! the project's normal release channel rather than committed into the plugin.
//! No per-OS/arch binary is bundled. `${CLAUDE_PLUGIN_ROOT}`-relative or
//! launcher-based strategies would only change how this one `command` field is
//! computed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use swissarmyhammer_agents::AgentResolver;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::deploy::format_skill_md;
use swissarmyhammer_skills::SkillResolver;

use crate::install::{
    profile_template_context, render_profile_agent, render_profile_skill, Profile,
    ProfileMcpServer, Selector,
};
use crate::registry::RegistryError;

/// The default output root for `mirdan plugin build`.
pub const DEFAULT_OUT_DIR: &str = "dist/plugins";

/// Human-facing metadata written into a plugin's `plugin.json`.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// Plugin name — also the plugin directory name and the MCP server key.
    pub name: String,
    /// Plugin version (the workspace version).
    pub version: String,
    /// One-line description shown in marketplaces and `/plugin`.
    pub description: String,
    /// Author name written under `author.name`.
    pub author: String,
}

/// A plugin to render: its metadata plus the [`Profile`] whose MCP server,
/// skills, and agents become the plugin's content.
#[derive(Debug, Clone)]
pub struct PluginSpec {
    /// Metadata for `plugin.json`.
    pub meta: PluginMeta,
    /// The declarative manifest of what the plugin contains.
    pub profile: Profile,
}

/// Summary of one rendered plugin, for reporting.
#[derive(Debug, Clone, Serialize)]
pub struct RenderedPlugin {
    /// Plugin name.
    pub name: String,
    /// Absolute path to the rendered plugin directory.
    pub path: PathBuf,
    /// MCP server keys written into `plugin.json` (strategy A: PATH `command`).
    pub mcp_servers: Vec<String>,
    /// Skill names written under `skills/`.
    pub skills: Vec<String>,
    /// Agent names written under `agents/`.
    pub agents: Vec<String>,
}

/// The author written into every SwissArmyHammer plugin manifest.
const AUTHOR: &str = "SwissArmyHammer Team";

/// The canonical set of CLI plugins, one per `serve`-capable binary.
///
/// This is the single source of truth for "all our CLIs," promoted from the
/// per-CLI `registry.rs::profile()` functions (and mirrored by mirdan's
/// `profile_consistency_tests`). Each entry's [`Profile`] is constructed from
/// the same public mirdan primitives those consumers use, so the packaged
/// plugin selects exactly the skills/agents `<cli> init` deploys.
///
/// Skills are included unconditionally here (the plugin is content-bearing by
/// definition); the `User`-scope "MCP only, no skills" gate the CLIs apply at
/// install time does not apply to packaging.
pub fn plugin_catalog() -> Vec<PluginSpec> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let meta = |name: &str, description: &str| PluginMeta {
        name: name.to_string(),
        version: version.clone(),
        description: description.to_string(),
        author: AUTHOR.to_string(),
    };

    vec![
        // sah — the umbrella "bigger profile": all builtin skills + all agents.
        PluginSpec {
            meta: meta(
                "sah",
                "SwissArmyHammer — full toolset MCP server with every builtin skill and subagent.",
            ),
            profile: Profile {
                mcp_server: Some(ProfileMcpServer::serve("sah")),
                skills: Some(Selector::All),
                agents: Some(Selector::All),
                validators: Some(Selector::All),
                statusline: false,
                preamble: false,
            },
        },
        // kanban — the `kanban` MCP server + the kanban-profile skill cluster
        // (kanban, plan, task, finish, implement, review).
        PluginSpec {
            meta: meta(
                "kanban",
                "Kanban board MCP server plus the kanban workflow skill cluster.",
            ),
            profile: Profile {
                mcp_server: Some(ProfileMcpServer::serve("kanban")),
                skills: Some(Selector::Profile("kanban".to_string())),
                ..Profile::default()
            },
        },
        // code-context — the `code-context` MCP server + code-context & lsp skills.
        PluginSpec {
            meta: meta(
                "code-context",
                "Structural code-intelligence MCP server with the code-context and lsp skills.",
            ),
            profile: Profile {
                mcp_server: Some(ProfileMcpServer::serve("code-context")),
                skills: Some(Selector::Named(vec![
                    "code-context".to_string(),
                    "lsp".to_string(),
                ])),
                ..Profile::default()
            },
        },
        // shelltool — the `shelltool` MCP server + the single `shell` skill.
        PluginSpec {
            meta: meta(
                "shelltool",
                "Sandboxed shell MCP server with the shell skill.",
            ),
            profile: Profile {
                mcp_server: Some(ProfileMcpServer::serve("shelltool")),
                skills: Some(Selector::Single("shell".to_string())),
                ..Profile::default()
            },
        },
    ]
}

/// Render every plugin in [`plugin_catalog`] into `out_root/<name>/`.
///
/// Each plugin directory is removed and rebuilt so the output is a clean,
/// reproducible snapshot of the current builtins.
pub fn build_all(out_root: &Path) -> Result<Vec<RenderedPlugin>, RegistryError> {
    let mut rendered = Vec::new();
    for spec in plugin_catalog() {
        rendered.push(render_plugin(&spec, out_root)?);
    }
    Ok(rendered)
}

/// Render a single [`PluginSpec`] into `out_root/<name>/`, returning a summary.
///
/// Writes `.claude-plugin/plugin.json`, then `skills/<name>/SKILL.md` (plus any
/// bundled skill resource files) for each selected skill and `agents/<name>.md`
/// for each selected agent. The target plugin directory is removed first so the
/// render is idempotent.
pub fn render_plugin(
    spec: &PluginSpec,
    out_root: &Path,
) -> Result<RenderedPlugin, RegistryError> {
    let plugin_dir = out_root.join(&spec.meta.name);
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
            RegistryError::Validation(format!(
                "failed to clean {}: {e}",
                plugin_dir.display()
            ))
        })?;
    }
    std::fs::create_dir_all(plugin_dir.join(".claude-plugin")).map_err(|e| {
        RegistryError::Validation(format!("failed to create plugin dir: {e}"))
    })?;

    let mcp_servers = write_plugin_json(spec, &plugin_dir)?;
    let skills = render_skills(spec.profile.skills.as_ref(), &plugin_dir)?;
    let agents = render_agents(spec.profile.agents.as_ref(), &plugin_dir)?;

    Ok(RenderedPlugin {
        name: spec.meta.name.clone(),
        path: plugin_dir,
        mcp_servers,
        skills,
        agents,
    })
}

/// Write `.claude-plugin/plugin.json`, returning the MCP server keys written.
///
/// Strategy A (PATH): the `mcpServers` entry is the profile's
/// [`ProfileMcpServer`] verbatim — `command` is the bare binary name, resolved
/// from `PATH` at runtime, with no bundled binary or `${CLAUDE_PLUGIN_ROOT}`
/// indirection.
fn write_plugin_json(
    spec: &PluginSpec,
    plugin_dir: &Path,
) -> Result<Vec<String>, RegistryError> {
    let mut manifest = json!({
        "name": spec.meta.name,
        "version": spec.meta.version,
        "description": spec.meta.description,
        "author": { "name": spec.meta.author },
    });

    let mut server_keys = Vec::new();
    if let Some(server) = &spec.profile.mcp_server {
        let mut servers = serde_json::Map::new();
        servers.insert(
            server.name.clone(),
            json!({ "command": server.command, "args": server.args }),
        );
        server_keys.push(server.name.clone());
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert("mcpServers".to_string(), serde_json::Value::Object(servers));
    }

    let path = plugin_dir.join(".claude-plugin").join("plugin.json");
    let body = serde_json::to_string_pretty(&manifest)
        .map_err(|e| RegistryError::Validation(format!("failed to serialize plugin.json: {e}")))?;
    std::fs::write(&path, body + "\n")
        .map_err(|e| RegistryError::Validation(format!("failed to write plugin.json: {e}")))?;

    Ok(server_keys)
}

/// Render the profile's selected builtin skills into `plugin_dir/skills/`.
fn render_skills(
    selector: Option<&Selector>,
    plugin_dir: &Path,
) -> Result<Vec<String>, RegistryError> {
    let Some(selector) = selector else {
        return Ok(Vec::new());
    };

    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    let available: HashMap<String, Vec<String>> = builtins
        .iter()
        .map(|(name, skill)| (name.clone(), skill.profiles.clone()))
        .collect();

    let library = PromptLibrary::default();
    let ctx = profile_template_context();

    let mut written = Vec::new();
    for name in selector.select(&available) {
        let skill = &builtins[&name];
        let (instructions, metadata) = render_profile_skill(&library, &ctx, skill);
        let content = format_skill_md(skill, &instructions, &metadata);

        let skill_dir = plugin_dir.join("skills").join(&name);
        write_item_file(&skill_dir, "SKILL.md", &content)?;
        write_resources(&skill_dir, &skill.resources.files)?;
        written.push(name);
    }
    Ok(written)
}

/// Render the profile's selected builtin agents into `plugin_dir/agents/`.
///
/// Claude Code discovers plugin agents as `agents/<name>.md`. Agents carry no
/// profile tags, so only `All`/`Named`/`Single` selectors match.
fn render_agents(
    selector: Option<&Selector>,
    plugin_dir: &Path,
) -> Result<Vec<String>, RegistryError> {
    let Some(selector) = selector else {
        return Ok(Vec::new());
    };

    let resolver = AgentResolver::new();
    let builtins = resolver.resolve_builtins();
    let available: HashMap<String, Vec<String>> =
        builtins.keys().map(|name| (name.clone(), Vec::new())).collect();

    let library = PromptLibrary::default();
    let ctx = profile_template_context();

    let agents_dir = plugin_dir.join("agents");
    let mut written = Vec::new();
    for name in selector.select(&available) {
        let agent = &builtins[&name];
        let content = render_profile_agent(&library, &ctx, agent);
        write_item_file(&agents_dir, &format!("{name}.md"), &content)?;
        write_resources(&agents_dir.join(&name), &agent.resources.files)?;
        written.push(name);
    }
    Ok(written)
}

/// Write `dir/file_name` with `content`, creating `dir` first.
fn write_item_file(dir: &Path, file_name: &str, content: &str) -> Result<(), RegistryError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| RegistryError::Validation(format!("failed to create {}: {e}", dir.display())))?;
    std::fs::write(dir.join(file_name), content).map_err(|e| {
        RegistryError::Validation(format!("failed to write {file_name}: {e}"))
    })
}

/// Write a skill/agent's bundled resource files under `base_dir`, preserving
/// any nested relative paths (e.g. `scripts/foo.sh`, `references/REF.md`).
fn write_resources(
    base_dir: &Path,
    files: &HashMap<String, String>,
) -> Result<(), RegistryError> {
    for (rel, body) in files {
        let dest = base_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RegistryError::Validation(format!("failed to create {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&dest, body).map_err(|e| {
            RegistryError::Validation(format!("failed to write {}: {e}", dest.display()))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_all_serve_clis() {
        let names: Vec<&str> = plugin_catalog()
            .iter()
            .map(|p| p.meta.name.as_str())
            .collect();
        // Every CLI that exposes `<name> serve` has a plugin.
        for expected in ["sah", "kanban", "code-context", "shelltool"] {
            assert!(names.contains(&expected), "missing plugin: {expected}");
        }
    }

    #[test]
    fn catalog_uses_path_strategy_for_mcp_command() {
        // Strategy A: command is the bare binary name (resolved from PATH),
        // launched with `serve`. No bundled-binary / plugin-root indirection.
        for spec in plugin_catalog() {
            let server = spec
                .profile
                .mcp_server
                .expect("every CLI plugin declares an MCP server");
            assert_eq!(server.command, spec.meta.name);
            assert_eq!(server.args, vec!["serve".to_string()]);
        }
    }

    #[test]
    fn render_plugin_writes_manifest_and_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = plugin_catalog()
            .into_iter()
            .find(|p| p.meta.name == "shelltool")
            .unwrap();

        let rendered = render_plugin(&spec, tmp.path()).unwrap();

        // Manifest exists with the PATH-strategy mcpServers entry.
        let manifest_path = rendered.path.join(".claude-plugin").join("plugin.json");
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["name"], "shelltool");
        assert_eq!(manifest["mcpServers"]["shelltool"]["command"], "shelltool");
        assert_eq!(manifest["mcpServers"]["shelltool"]["args"][0], "serve");

        // The `shell` skill was rendered into skills/shell/SKILL.md.
        assert_eq!(rendered.skills, vec!["shell".to_string()]);
        assert!(rendered
            .path
            .join("skills")
            .join("shell")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn render_plugin_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = plugin_catalog()
            .into_iter()
            .find(|p| p.meta.name == "shelltool")
            .unwrap();
        let first = render_plugin(&spec, tmp.path()).unwrap();
        let second = render_plugin(&spec, tmp.path()).unwrap();
        assert_eq!(first.skills, second.skills);
        assert!(second.path.join(".claude-plugin").join("plugin.json").exists());
    }
}
