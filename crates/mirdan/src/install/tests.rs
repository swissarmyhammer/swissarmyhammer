use swissarmyhammer_common::lifecycle::InitScope;

use crate::lockfile::{LockedPackage, Lockfile};
use crate::mcp_config;
use crate::package_type::{self, PackageType};
use crate::registry::RegistryError;

use super::*;
use serial_test::serial;
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

#[test]
fn test_find_packages_by_git_source_uppercase_scheme_and_host() {
    let mut lf = Lockfile::default();
    lf.add_package(
        "anthropics/skills/algorithmic-art".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "0.0.0".to_string(),
            resolved: "git+https://github.com/anthropics/skills.git".to_string(),
            integrity: String::new(),
            installed_at: String::new(),
            targets: Vec::new(),
        },
    );

    // `Url::parse` normalizes the scheme and host to lowercase, so an
    // uppercase scheme/host must match the lowercase `resolved` entry.
    let matched = find_packages_by_git_source(&lf, "HTTPS://GITHUB.COM/anthropics/skills");
    assert_eq!(
        matched,
        vec!["anthropics/skills/algorithmic-art".to_string()]
    );
}

#[test]
fn test_parse_package_spec_name_only() {
    let (name, version) = parse_package_spec("no-secrets");
    assert_eq!(name, "no-secrets");
    assert_eq!(version, None);
}

#[test]
fn test_parse_package_spec_with_version() {
    let (name, version) = parse_package_spec("no-secrets@1.2.3");
    assert_eq!(name, "no-secrets");
    assert_eq!(version, Some("1.2.3".to_string()));
}

#[test]
fn test_validators_dir_local() {
    let dir = validators_dir(false);
    assert_eq!(dir, PathBuf::from(".validators"));
}

#[test]
fn test_validators_dir_global() {
    // The global validators store is the home dotfile `~/.validators`,
    // resolved through the shared mirdan store mechanism — not XDG
    // `~/.local/share/validators`.
    let dir = validators_dir(true);
    assert!(dir.ends_with(".validators"));
    assert!(!dir.ends_with(".avp/validators"));
    let home = dirs::home_dir().unwrap();
    assert!(dir.starts_with(home));
}

/// A profile that only materializes every builtin validator set — no MCP
/// server, skills, or agents — so the validators path is exercised in
/// isolation (no agent detection required).
fn validators_only_profile() -> Profile {
    Profile {
        validators: Some(Selector::All),
        ..Profile::default()
    }
}

/// `init_profile` materializes the embedded builtin validators under
/// `~/.validators/<set>/` (the global store, via the shared mirdan store
/// mechanism) with their full structure (VALIDATOR.md + rules/*.md),
/// matching the embedded source.
#[test]
#[serial(cwd)]
fn init_profile_materializes_builtin_validators_to_home_store() {
    let env = IsolatedTestEnvironment::new().unwrap();

    let reporter = NullReporter;
    let results = init_profile(&validators_only_profile(), InitScope::User, None, &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "init_profile must not error: {results:?}"
    );

    let validators_root = env.home_path().join(".validators");
    // Every embedded builtin set is materialized with its manifest. The nine
    // single-rule sets (no-secrets, injection, command-safety, etc.) were
    // merged into the focused code-security / code-hygiene sets.
    for set in ["code-hygiene", "code-security", "test-integrity"] {
        let manifest = validators_root.join(set).join("VALIDATOR.md");
        assert!(
            manifest.is_file(),
            "builtin set `{set}` must materialize a VALIDATOR.md at {manifest:?}"
        );
    }

    // A nested rule file is materialized and matches the embedded content.
    let rule = validators_root.join("code-hygiene/rules/dead-code.md");
    assert!(
        rule.is_file(),
        "nested rule file must materialize: {rule:?}"
    );
    let embedded = crate::builtin_validators::get_builtin_validators()
        .into_iter()
        .find(|(name, _)| *name == "code-hygiene/rules/dead-code.md")
        .map(|(_, content)| content)
        .expect("embedded builtin must include code-hygiene/rules/dead-code.md");
    assert_eq!(
        std::fs::read_to_string(&rule).unwrap(),
        embedded,
        "materialized rule must byte-match the embedded source"
    );
}

/// Reference-copy policy: re-running the install is idempotent and
/// authoritative for builtin-owned files (a hand-edited builtin file is
/// restored to embedded content), while user-authored validators and
/// user-created sets under the same directory survive untouched.
#[test]
#[serial(cwd)]
fn init_profile_validators_idempotent_refreshes_builtin_preserves_user() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let reporter = NullReporter;
    let validators_root = env.home_path().join(".validators");

    // First install.
    init_profile(&validators_only_profile(), InitScope::User, None, &reporter);

    // 1. Hand-edit a builtin-deployed file (simulating a user tampering with
    //    the read-only reference copy).
    let tampered = validators_root.join("code-hygiene/VALIDATOR.md");
    std::fs::write(&tampered, "TAMPERED").unwrap();

    // 2. Add a user-authored validator *inside* a builtin set dir.
    let user_file_in_builtin = validators_root.join("code-hygiene/rules/my-rule.md");
    std::fs::write(&user_file_in_builtin, "USER RULE").unwrap();

    // 3. Add an entirely user-created set.
    let user_set = validators_root.join("my-team-rules");
    std::fs::create_dir_all(&user_set).unwrap();
    let user_set_manifest = user_set.join("VALIDATOR.md");
    std::fs::write(&user_set_manifest, "USER SET").unwrap();

    // Second install — idempotent refresh.
    let results = init_profile(&validators_only_profile(), InitScope::User, None, &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "second install must not error: {results:?}"
    );

    // The tampered builtin file is restored to the embedded content.
    let embedded_manifest = crate::builtin_validators::get_builtin_validators()
        .into_iter()
        .find(|(name, _)| *name == "code-hygiene/VALIDATOR.md")
        .map(|(_, content)| content)
        .expect("embedded builtin must include code-hygiene/VALIDATOR.md");
    assert_eq!(
        std::fs::read_to_string(&tampered).unwrap(),
        embedded_manifest,
        "builtin-owned file must be refreshed/overwritten on reinstall"
    );

    // The user-authored validator inside the builtin set survives untouched.
    assert_eq!(
        std::fs::read_to_string(&user_file_in_builtin).unwrap(),
        "USER RULE",
        "a user file added under a builtin set must never be touched"
    );

    // The user-created set survives untouched.
    assert_eq!(
        std::fs::read_to_string(&user_set_manifest).unwrap(),
        "USER SET",
        "a user-created set must never be touched"
    );
}

/// A refresh (re-running `init_profile`) prunes a retired builtin set from
/// the deployed store when its files are byte-identical to what was
/// shipped before it was retired, but leaves a user-modified copy of the
/// same retired set alone.
#[test]
#[serial(cwd)]
fn init_profile_refresh_prunes_unmodified_retired_set_but_keeps_user_modified_copy() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let reporter = NullReporter;
    let validators_root = env.home_path().join(".validators");

    // Deploy the OLD (pre-merge) `no-secrets` set exactly as it shipped,
    // simulating a store populated by a binary from before the merge.
    let no_secrets_set = crate::retired_validators::RETIRED_VALIDATOR_SETS
        .iter()
        .find(|s| s.name == "no-secrets")
        .expect("no-secrets is a retired set");
    for file in no_secrets_set.files {
        let dest = validators_root.join("no-secrets").join(file.relative_path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, file.content).unwrap();
    }

    // Deploy a user-MODIFIED copy of a different retired set (`injection`).
    let injection_set = crate::retired_validators::RETIRED_VALIDATOR_SETS
        .iter()
        .find(|s| s.name == "injection")
        .expect("injection is a retired set");
    for file in injection_set.files {
        let dest = validators_root.join("injection").join(file.relative_path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, file.content).unwrap();
    }
    let injection_manifest = validators_root.join("injection/VALIDATOR.md");
    std::fs::write(&injection_manifest, "USER MODIFIED THIS FILE").unwrap();

    // Run the refresh.
    let results = init_profile(&validators_only_profile(), InitScope::User, None, &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "refresh must not error: {results:?}"
    );

    // The unmodified retired `no-secrets` set is gone entirely.
    assert!(
        !validators_root.join("no-secrets").exists(),
        "an unmodified retired set must be pruned by refresh"
    );

    // The user-modified retired `injection` set survives, edit intact.
    assert!(
        validators_root.join("injection").is_dir(),
        "a user-modified retired set must survive refresh"
    );
    assert_eq!(
        std::fs::read_to_string(&injection_manifest).unwrap(),
        "USER MODIFIED THIS FILE",
        "the user's edit to a retired set must be preserved exactly"
    );
}

/// `init_profile` drops the builtin discovery `README.md` at the store root,
/// it is not mistaken for a validator set, and `deinit_profile` removes it.
#[test]
#[serial(cwd)]
fn init_profile_writes_store_readme_and_deinit_removes_it() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let reporter = NullReporter;
    let validators_root = env.home_path().join(".validators");

    init_profile(&validators_only_profile(), InitScope::User, None, &reporter);

    // The README is written at the store root with the embedded content.
    let readme = validators_root.join("README.md");
    assert!(
        readme.is_file(),
        "init must write a discovery README at the validator store root: {readme:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&readme).unwrap(),
        include_str!("../../../../builtin/validators/README.md"),
        "the deployed README must match the builtin source content"
    );

    // The README sits beside the set subdirectories and is never treated as a
    // set — the loader only considers subdirectories, so it has no VALIDATOR.md
    // child and the real builtin sets still materialize alongside it.
    assert!(readme.is_file() && !readme.is_dir());
    assert!(
        validators_root.join("code-hygiene/VALIDATOR.md").is_file(),
        "real builtin sets must materialize beside the README"
    );

    // A user-authored set keeps the store directory alive across deinit, but
    // the builtin README is still removed (it is builtin-owned).
    let user_set_manifest = validators_root.join("my-team-rules/VALIDATOR.md");
    std::fs::create_dir_all(user_set_manifest.parent().unwrap()).unwrap();
    std::fs::write(&user_set_manifest, "USER SET").unwrap();

    deinit_profile(&validators_only_profile(), InitScope::User, None, &reporter);

    assert!(
        !readme.exists(),
        "deinit must remove the builtin discovery README"
    );
    assert_eq!(
        std::fs::read_to_string(&user_set_manifest).unwrap(),
        "USER SET",
        "deinit must leave user-authored sets untouched"
    );
}

#[test]
fn test_sanitize_dir_name_url() {
    assert_eq!(
        sanitize_dir_name("https://github.com/anthropics/skills/algorithmic-art"),
        "anthropics/skills/algorithmic-art"
    );
}

#[test]
fn test_sanitize_dir_name_http() {
    assert_eq!(sanitize_dir_name("http://example.com/foo/bar"), "foo/bar");
}

#[test]
fn test_sanitize_dir_name_plain() {
    assert_eq!(sanitize_dir_name("no-secrets"), "no-secrets");
}

#[test]
fn test_sanitize_dir_name_host_only() {
    assert_eq!(sanitize_dir_name("https://github.com"), "github.com");
}

#[test]
fn test_read_frontmatter_skill() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(
        &md,
        "---\nname: test-skill\nmetadata:\n  version: \"1.2.3\"\n---\n# Test\n",
    )
    .unwrap();

    let (name, version) = read_frontmatter(&md).unwrap();
    assert_eq!(name, "test-skill");
    assert_eq!(version, "1.2.3");
}

#[test]
fn test_read_frontmatter_missing_version_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(&md, "---\nname: test-skill\n---\n# Test\n").unwrap();

    let (name, version) = read_frontmatter(&md).unwrap();
    assert_eq!(name, "test-skill");
    assert_eq!(version, "0.0.0");
}

#[test]
fn test_read_frontmatter_metadata_version() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(
        &md,
        "---\nname: test-skill\nmetadata:\n  version: \"2.0.0\"\n---\n# Test\n",
    )
    .unwrap();

    let (name, version) = read_frontmatter(&md).unwrap();
    assert_eq!(name, "test-skill");
    assert_eq!(version, "2.0.0");
}

#[test]
fn test_read_frontmatter_metadata_preferred() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(
        &md,
        "---\nname: test-skill\nversion: \"1.0.0\"\nmetadata:\n  version: \"2.0.0\"\n---\n# Test\n",
    )
    .unwrap();

    let (name, version) = read_frontmatter(&md).unwrap();
    assert_eq!(name, "test-skill");
    assert_eq!(version, "2.0.0");
}

#[test]
fn test_read_frontmatter_missing_name_errors() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(&md, "---\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n").unwrap();

    assert!(read_frontmatter(&md).is_err());
}

#[test]
fn test_read_frontmatter_no_frontmatter_errors() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("SKILL.md");
    std::fs::write(&md, "# Just markdown\nNo frontmatter here.\n").unwrap();

    assert!(read_frontmatter(&md).is_err());
}

#[test]
fn test_stage_and_deploy_skill_rejects_traversal() {
    let err = stage_and_deploy_skill("../escape", "# Skill\n").unwrap_err();
    assert!(matches!(err, RegistryError::Validation(_)));
}

#[test]
fn test_copy_dir_recursive() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    let dst_path = dst.path().join("copy");

    std::fs::write(src.path().join("file.txt"), "hello").unwrap();
    std::fs::create_dir(src.path().join("sub")).unwrap();
    std::fs::write(src.path().join("sub/nested.txt"), "world").unwrap();

    copy_dir_recursive(src.path(), &dst_path).unwrap();

    assert!(dst_path.join("file.txt").exists());
    assert!(dst_path.join("sub/nested.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dst_path.join("file.txt")).unwrap(),
        "hello"
    );
}

#[test]
fn remove_empty_dirs_up_to_climbs_arbitrary_depth_and_stops_at_nonempty() {
    let tmp = tempfile::tempdir().unwrap();
    let boundary = tmp.path().join("store");

    // Deeply nested empty tree below the boundary: store/set/a/b/c.
    let deep = boundary.join("set/a/b/c");
    std::fs::create_dir_all(&deep).unwrap();

    // A sibling set that holds a user file must survive untouched.
    let user_set = boundary.join("user-set");
    std::fs::create_dir_all(&user_set).unwrap();
    std::fs::write(user_set.join("keep.md"), "USER").unwrap();

    remove_empty_dirs_up_to(&deep, &boundary);

    // Every empty intermediate directory up to (but not including) the
    // boundary is gone — not just the leaf and its immediate parent.
    assert!(!boundary.join("set/a/b/c").exists());
    assert!(!boundary.join("set/a/b").exists());
    assert!(!boundary.join("set/a").exists());
    assert!(!boundary.join("set").exists());

    // The boundary itself is never removed, and a non-empty sibling set is
    // preserved (the non-empty guard stops the climb at the right place).
    assert!(boundary.exists(), "boundary must be preserved");
    assert!(
        user_set.join("keep.md").exists(),
        "a set holding user files must survive"
    );
}

#[test]
fn remove_empty_dirs_up_to_halts_at_first_nonempty_ancestor() {
    let tmp = tempfile::tempdir().unwrap();
    let boundary = tmp.path().join("store");

    // store/set/rules/<empty leaf>, but the set dir also holds a user file,
    // so the climb must stop once it reaches the non-empty `set` dir.
    let leaf = boundary.join("set/rules");
    std::fs::create_dir_all(&leaf).unwrap();
    std::fs::write(boundary.join("set/user.md"), "USER").unwrap();

    remove_empty_dirs_up_to(&leaf, &boundary);

    assert!(
        !boundary.join("set/rules").exists(),
        "empty leaf is removed"
    );
    assert!(
        boundary.join("set").exists(),
        "non-empty set dir halts the climb"
    );
    assert!(boundary.join("set/user.md").exists(), "user file survives");
}

// --- local skill: create, detect, read frontmatter, deploy as validator ---

/// Helper: create a local skill directory with SKILL.md.
fn make_local_skill(dir: &Path, name: &str, version: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {}\nmetadata:\n  version: \"{}\"\n---\n# {}\nA test skill.\n",
            name, version, name
        ),
    )
    .unwrap();
}

/// Helper: create a local validator directory with VALIDATOR.md + rules/.
fn make_local_validator(dir: &Path, name: &str, version: &str) {
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(
        dir.join("VALIDATOR.md"),
        format!(
            "---\nname: {}\nmetadata:\n  version: \"{}\"\n---\n# {}\nA test validator.\n",
            name, version, name
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("rules/no-secrets.md"),
        "# No Secrets\nDon't commit secrets.\n",
    )
    .unwrap();
}

/// Helper: create a local tool directory with a realistic TOOL.md.
///
/// Uses @modelcontextprotocol/server-filesystem as the MCP server —
/// a real, published npm package that implements the MCP protocol for
/// filesystem access.
fn make_local_tool(dir: &Path, name: &str, version: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("TOOL.md"),
        format!(
            r#"---
name: {name}
description: "MCP server for filesystem access"
metadata:
  version: "{version}"
mcp:
  command: npx
  args:
    - "-y"
    - "@modelcontextprotocol/server-filesystem"
    - "/tmp/safe-dir"
  transport: stdio
  env:
    NODE_ENV: production
---

# {name}

An MCP tool that provides filesystem access to AI coding agents
via the Model Context Protocol.

## What This Tool Does

Exposes read/write filesystem operations through MCP so agents can
work with files in a controlled directory.
"#,
            name = name,
            version = version,
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("README.md"),
        format!("# {}\n\nAn MCP filesystem tool.\n", name),
    )
    .unwrap();
}

/// Helper: create a local plugin directory with .claude-plugin/plugin.json.
///
/// Creates a realistic Claude Code plugin with a command and an
/// optional bundled .mcp.json.
fn make_local_plugin(dir: &Path, name: &str, with_mcp: bool) {
    let plugin_meta = dir.join(".claude-plugin");
    let commands_dir = dir.join("commands");
    std::fs::create_dir_all(&plugin_meta).unwrap();
    std::fs::create_dir_all(&commands_dir).unwrap();

    std::fs::write(
        plugin_meta.join("plugin.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": name,
            "description": "A test plugin for e2e testing",
            "author": { "name": "test" }
        }))
        .unwrap(),
    )
    .unwrap();

    std::fs::write(
        commands_dir.join("greet.md"),
        format!(
            "---\ndescription: \"Say hello from {name}\"\nallowed-tools:\n  - Read\n---\n\n\
                 # Greet\n\nSay hello to the user.\n",
            name = name,
        ),
    )
    .unwrap();

    if with_mcp {
        std::fs::write(
            dir.join(".mcp.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    format!("{}-server", name): {
                        "command": "node",
                        "args": ["./server.js"]
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    std::fs::write(
        dir.join("README.md"),
        format!("# {}\n\nA test plugin.\n", name),
    )
    .unwrap();
}

// --- tool: detection, frontmatter, deploy, uninstall ---

#[test]
fn test_local_tool_detection_and_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let tool_dir = dir.path().join("fs-tool");
    make_local_tool(&tool_dir, "fs-tool", "1.0.0");

    // detect_package_type recognises it as a Tool
    let pkg_type = package_type::detect_package_type(&tool_dir);
    assert_eq!(pkg_type, Some(PackageType::Tool));

    // read_frontmatter extracts name + version
    let (name, version) = read_frontmatter(&tool_dir.join("TOOL.md")).unwrap();
    assert_eq!(name, "fs-tool");
    assert_eq!(version, "1.0.0");

    // MCP frontmatter parses correctly
    let yaml = mcp_config::parse_yaml_frontmatter(&tool_dir.join("TOOL.md")).unwrap();
    let mcp_fm = mcp_config::parse_tool_frontmatter(&yaml).unwrap();
    assert_eq!(mcp_fm.command, "npx");
    assert_eq!(
        mcp_fm.args,
        vec![
            "-y",
            "@modelcontextprotocol/server-filesystem",
            "/tmp/safe-dir"
        ]
    );
    assert_eq!(mcp_fm.transport, Some("stdio".to_string()));
    assert_eq!(mcp_fm.env.get("NODE_ENV").unwrap(), "production");
}

#[test]
#[serial]
fn test_deploy_tool_creates_store_and_mcp_json() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Create a source tool with a real MCP server reference
    let src = work.path().join("src-tool");
    make_local_tool(&src, "fs-tool", "1.0.0");

    // Deploy it (non-global)
    let targets = deploy_tool("fs-tool", &src, None, false).unwrap();
    // claude-code has mcp_config, so it should be a target
    assert!(
        targets.contains(&"claude-code".to_string()),
        "claude-code should be in targets: {:?}",
        targets
    );

    // 1. Verify tool store: .tools/fs-tool/ has TOOL.md + README.md
    let store = work.path().join(".tools/fs-tool");
    assert!(store.join("TOOL.md").exists(), "TOOL.md should be in store");
    assert!(
        store.join("README.md").exists(),
        "README.md should be in store"
    );

    // Verify the stored TOOL.md is byte-identical to the source
    let src_content = std::fs::read_to_string(src.join("TOOL.md")).unwrap();
    let store_content = std::fs::read_to_string(store.join("TOOL.md")).unwrap();
    assert_eq!(src_content, store_content, "Store copy should match source");

    // 2. Verify .mcp.json was created with the correct MCP server entry
    let mcp_json_path = work.path().join(".mcp.json");
    assert!(mcp_json_path.exists(), ".mcp.json should exist");
    let mcp_content = std::fs::read_to_string(&mcp_json_path).unwrap();
    let mcp_json: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();

    // The entry should be under mcpServers.fs-tool
    let server = &mcp_json["mcpServers"]["fs-tool"];
    assert_eq!(
        server["command"].as_str().unwrap(),
        "npx",
        "command should be npx"
    );
    let args: Vec<&str> = server["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        args,
        vec![
            "-y",
            "@modelcontextprotocol/server-filesystem",
            "/tmp/safe-dir"
        ],
        "args should match TOOL.md"
    );
    assert_eq!(
        server["env"]["NODE_ENV"].as_str().unwrap(),
        "production",
        "env should be passed through"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_and_uninstall_tool() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Deploy
    let src = work.path().join("src-tool");
    make_local_tool(&src, "fs-tool", "1.0.0");
    deploy_tool("fs-tool", &src, None, false).unwrap();

    let store = work.path().join(".tools/fs-tool");
    let mcp_json_path = work.path().join(".mcp.json");
    assert!(store.exists());
    assert!(mcp_json_path.exists());

    // Verify server is registered before uninstall
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
    assert!(
        mcp["mcpServers"]["fs-tool"].is_object(),
        "Server should be registered"
    );

    // Uninstall
    uninstall_tool("fs-tool", None, false).unwrap();

    // Store entry should be gone
    assert!(!store.exists(), "Tool store entry should be removed");

    // MCP server entry should be gone, but mcpServers key remains
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
    assert!(
        mcp["mcpServers"]["fs-tool"].is_null(),
        "Server entry should be removed from .mcp.json"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_tool_preserves_existing_mcp_servers() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Pre-populate .mcp.json with an existing server
    std::fs::write(
        work.path().join(".mcp.json"),
        r#"{
  "mcpServers": {
    "existing-server": {
      "command": "node",
      "args": ["./existing.js"]
    }
  }
}"#,
    )
    .unwrap();

    // Deploy a new tool
    let src = work.path().join("src-tool");
    make_local_tool(&src, "fs-tool", "1.0.0");
    deploy_tool("fs-tool", &src, None, false).unwrap();

    // Both servers should be present
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert_eq!(
        mcp["mcpServers"]["existing-server"]["command"]
            .as_str()
            .unwrap(),
        "node",
        "Existing server should be preserved"
    );
    assert_eq!(
        mcp["mcpServers"]["fs-tool"]["command"].as_str().unwrap(),
        "npx",
        "New tool should be added"
    );

    // Uninstall only the new tool
    uninstall_tool("fs-tool", None, false).unwrap();

    // Existing server should still be there
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert_eq!(
        mcp["mcpServers"]["existing-server"]["command"]
            .as_str()
            .unwrap(),
        "node",
        "Existing server should survive uninstall of other tool"
    );
    assert!(
        mcp["mcpServers"]["fs-tool"].is_null(),
        "Uninstalled tool should be gone"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_uninstall_tool_not_found() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let result = uninstall_tool("nonexistent-tool", None, false);
    assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

    std::env::set_current_dir(old_dir).unwrap();
}

// --- plugin: detection, deploy, uninstall ---

#[test]
fn test_local_plugin_detection() {
    let dir = tempfile::tempdir().unwrap();
    let plugin_dir = dir.path().join("my-plugin");
    make_local_plugin(&plugin_dir, "my-plugin", false);

    let pkg_type = package_type::detect_package_type(&plugin_dir);
    assert_eq!(pkg_type, Some(PackageType::Plugin));

    // Read name from plugin.json
    let name =
        mcp_config::read_plugin_json(&plugin_dir.join(".claude-plugin/plugin.json")).unwrap();
    assert_eq!(name, "my-plugin");
}

#[test]
#[serial]
fn test_deploy_plugin_creates_files() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let src = work.path().join("src-plugin");
    make_local_plugin(&src, "test-plugin", false);

    let targets = deploy_plugin("test-plugin", &src, None, false).unwrap();
    assert!(
        targets.contains(&"claude-code".to_string()),
        "claude-code should be in targets: {:?}",
        targets
    );

    // Verify the plugin was copied to .claude/plugins/test-plugin/
    let deployed = work.path().join(".claude/plugins/test-plugin");
    assert!(deployed.exists(), "Plugin dir should exist");
    assert!(
        deployed.join(".claude-plugin/plugin.json").exists(),
        "plugin.json should be deployed"
    );
    assert!(
        deployed.join("commands/greet.md").exists(),
        "Commands should be deployed"
    );
    assert!(
        deployed.join("README.md").exists(),
        "README should be deployed"
    );

    // Verify plugin.json content is preserved
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(deployed.join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(json["name"].as_str().unwrap(), "test-plugin");

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_and_uninstall_plugin() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let src = work.path().join("src-plugin");
    make_local_plugin(&src, "test-plugin", false);
    deploy_plugin("test-plugin", &src, None, false).unwrap();

    let deployed = work.path().join(".claude/plugins/test-plugin");
    assert!(deployed.exists());

    uninstall_plugin("test-plugin", None, false).unwrap();
    assert!(!deployed.exists(), "Plugin dir should be removed");

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_plugin_with_bundled_mcp() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Create a plugin that bundles an .mcp.json
    let src = work.path().join("src-plugin");
    make_local_plugin(&src, "mcp-plugin", true);

    deploy_plugin("mcp-plugin", &src, None, false).unwrap();

    // Plugin should be deployed
    let deployed = work.path().join(".claude/plugins/mcp-plugin");
    assert!(deployed.exists());

    // The bundled .mcp.json servers should be registered in the
    // project-level .mcp.json (claude-code's mcp_config.project_path)
    let mcp_json_path = work.path().join(".mcp.json");
    assert!(
        mcp_json_path.exists(),
        ".mcp.json should be created from bundled MCP servers"
    );
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
    assert!(
        mcp["mcpServers"]["mcp-plugin-server"].is_object(),
        "Bundled MCP server should be registered"
    );
    assert_eq!(
        mcp["mcpServers"]["mcp-plugin-server"]["command"]
            .as_str()
            .unwrap(),
        "node"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_uninstall_plugin_not_found() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let result = uninstall_plugin("nonexistent-plugin", None, false);
    assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

    std::env::set_current_dir(old_dir).unwrap();
}

// --- e2e: tool install → lockfile → list → uninstall ---

#[test]
#[serial]
fn test_e2e_tool_install_list_uninstall() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // 1. Create and deploy a tool using @modelcontextprotocol/server-filesystem
    let src = work.path().join("src-tool");
    make_local_tool(&src, "fs-tool", "2.1.0");
    let targets = deploy_tool("fs-tool", &src, None, false).unwrap();

    // 2. Write lockfile (mimicking what run_install_local does)
    let mut lf = Lockfile::default();
    lf.add_package(
        "fs-tool".to_string(),
        LockedPackage {
            package_type: PackageType::Tool,
            version: "2.1.0".to_string(),
            resolved: format!("file:{}", src.display()),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.clone(),
        },
    );
    lf.save(work.path()).unwrap();

    // 3. Verify lockfile round-trip
    let loaded = Lockfile::load(work.path()).unwrap();
    let pkg = loaded.get_package("fs-tool").unwrap();
    assert_eq!(pkg.package_type, PackageType::Tool);
    assert_eq!(pkg.version, "2.1.0");

    // 4. Verify on-disk state
    assert!(work.path().join(".tools/fs-tool/TOOL.md").exists());
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert_eq!(mcp["mcpServers"]["fs-tool"]["command"], "npx");

    // 5. Verify list discovers the tool
    let packages = crate::list::discover_packages(false, false, true, false, None);
    let tool_pkgs: Vec<_> = packages.iter().filter(|p| p.name == "fs-tool").collect();
    assert_eq!(tool_pkgs.len(), 1, "list --tools should find fs-tool");
    assert_eq!(tool_pkgs[0].package_type, PackageType::Tool);
    assert_eq!(tool_pkgs[0].version, "2.1.0");

    // 6. Uninstall and verify cleanup
    uninstall_tool("fs-tool", None, false).unwrap();
    assert!(!work.path().join(".tools/fs-tool").exists());
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert!(mcp["mcpServers"]["fs-tool"].is_null());

    // 7. Clean lockfile
    let mut lf = Lockfile::load(work.path()).unwrap();
    lf.remove_package("fs-tool");
    lf.save(work.path()).unwrap();
    let lf = Lockfile::load(work.path()).unwrap();
    assert!(lf.packages.is_empty());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_e2e_plugin_install_list_uninstall() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // 1. Create and deploy a plugin
    let src = work.path().join("src-plugin");
    make_local_plugin(&src, "test-plugin", false);
    let targets = deploy_plugin("test-plugin", &src, None, false).unwrap();

    // 2. Write lockfile
    let mut lf = Lockfile::default();
    lf.add_package(
        "test-plugin".to_string(),
        LockedPackage {
            package_type: PackageType::Plugin,
            version: "0.0.0".to_string(),
            resolved: format!("file:{}", src.display()),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.clone(),
        },
    );
    lf.save(work.path()).unwrap();

    // 3. Verify lockfile round-trip
    let loaded = Lockfile::load(work.path()).unwrap();
    let pkg = loaded.get_package("test-plugin").unwrap();
    assert_eq!(pkg.package_type, PackageType::Plugin);

    // 4. Verify on-disk state
    let deployed = work.path().join(".claude/plugins/test-plugin");
    assert!(deployed.join(".claude-plugin/plugin.json").exists());
    assert!(deployed.join("commands/greet.md").exists());

    // 5. Verify list discovers the plugin
    let packages = crate::list::discover_packages(false, false, false, true, None);
    let plugin_pkgs: Vec<_> = packages
        .iter()
        .filter(|p| p.name == "test-plugin")
        .collect();
    assert_eq!(
        plugin_pkgs.len(),
        1,
        "list --plugins should find test-plugin"
    );
    assert_eq!(plugin_pkgs[0].package_type, PackageType::Plugin);

    // 6. Uninstall and verify cleanup
    uninstall_plugin("test-plugin", None, false).unwrap();
    assert!(
        !deployed.exists(),
        "Plugin dir should be removed after uninstall"
    );

    // 7. Clean lockfile
    let mut lf = Lockfile::load(work.path()).unwrap();
    lf.remove_package("test-plugin");
    lf.save(work.path()).unwrap();
    let lf = Lockfile::load(work.path()).unwrap();
    assert!(lf.packages.is_empty());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
fn test_local_skill_detection_and_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("my-skill");
    make_local_skill(&skill_dir, "my-skill", "1.2.3");

    // detect_package_type recognises it as a Skill
    let pkg_type = package_type::detect_package_type(&skill_dir);
    assert_eq!(pkg_type, Some(PackageType::Skill));

    // read_frontmatter extracts name + version
    let (name, version) = read_frontmatter(&skill_dir.join("SKILL.md")).unwrap();
    assert_eq!(name, "my-skill");
    assert_eq!(version, "1.2.3");
}

#[test]
fn test_local_validator_detection_and_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let val_dir = dir.path().join("my-validator");
    make_local_validator(&val_dir, "my-validator", "0.1.0");

    let pkg_type = package_type::detect_package_type(&val_dir);
    assert_eq!(pkg_type, Some(PackageType::Validator));

    let (name, version) = read_frontmatter(&val_dir.join("VALIDATOR.md")).unwrap();
    assert_eq!(name, "my-validator");
    assert_eq!(version, "0.1.0");
}

// --- validator deploy + uninstall (no agents required) ---

#[test]
#[serial]
fn test_deploy_validator_creates_files() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Create a source validator
    let src = work.path().join("src-val");
    make_local_validator(&src, "test-val", "1.0.0");

    // Deploy it (non-global → .validators/)
    let targets = deploy_validator("test-val", &src, false).unwrap();
    assert_eq!(targets.len(), 1);

    // Verify files exist on disk
    let deployed = work.path().join(".validators/test-val");
    assert!(deployed.join("VALIDATOR.md").exists());
    assert!(deployed.join("rules/no-secrets.md").exists());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_and_uninstall_validator() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Deploy
    let src = work.path().join("src-val");
    make_local_validator(&src, "test-val", "1.0.0");
    deploy_validator("test-val", &src, false).unwrap();

    let deployed = work.path().join(".validators/test-val");
    assert!(deployed.exists());

    // Uninstall
    uninstall_validator("test-val", false).unwrap();
    assert!(!deployed.exists(), "Validator dir should be removed");

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_uninstall_validator_not_found() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let result = uninstall_validator("nonexistent", false);
    assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

    std::env::set_current_dir(old_dir).unwrap();
}

// --- lockfile round-trip for git-installed packages ---

#[test]
fn test_lockfile_records_git_source() {
    let work = tempfile::tempdir().unwrap();

    let mut lf = Lockfile::default();
    lf.add_package(
        "skill-a".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "1.0.0".to_string(),
            resolved: "git+https://github.com/anthropics/skills.git".to_string(),
            integrity: String::new(),
            installed_at: "2026-02-16T00:00:00Z".to_string(),
            targets: vec!["claude-code".to_string()],
        },
    );
    lf.add_package(
        "skill-b".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "1.0.0".to_string(),
            resolved: "git+https://github.com/anthropics/skills.git".to_string(),
            integrity: String::new(),
            installed_at: "2026-02-16T00:00:00Z".to_string(),
            targets: vec!["claude-code".to_string()],
        },
    );
    lf.add_package(
        "other-pkg".to_string(),
        LockedPackage {
            package_type: PackageType::Validator,
            version: "0.1.0".to_string(),
            resolved: "https://registry.example.com/other-pkg-0.1.0.zip".to_string(),
            integrity: "sha512-abc".to_string(),
            installed_at: "2026-02-16T00:00:00Z".to_string(),
            targets: vec![".validators/".to_string()],
        },
    );

    lf.save(work.path()).unwrap();
    let loaded = Lockfile::load(work.path()).unwrap();
    assert_eq!(loaded.packages.len(), 3);

    // git packages have empty integrity, git+ resolved prefix
    let a = loaded.get_package("skill-a").unwrap();
    assert!(a.resolved.starts_with("git+"));
    assert!(a.integrity.is_empty());

    // registry package has integrity
    let o = loaded.get_package("other-pkg").unwrap();
    assert!(!o.resolved.starts_with("git+"));
    assert!(!o.integrity.is_empty());
}

// --- uninstall-by-URL matching ---

#[test]
fn test_find_packages_by_git_url() {
    let mut lf = Lockfile::default();
    lf.add_package(
        "skill-a".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "1.0.0".to_string(),
            resolved: "git+https://github.com/anthropics/skills.git".to_string(),
            integrity: String::new(),
            installed_at: String::new(),
            targets: vec![],
        },
    );
    lf.add_package(
        "skill-b".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "1.0.0".to_string(),
            resolved: "git+https://github.com/anthropics/skills.git".to_string(),
            integrity: String::new(),
            installed_at: String::new(),
            targets: vec![],
        },
    );
    lf.add_package(
        "other-pkg".to_string(),
        LockedPackage {
            package_type: PackageType::Validator,
            version: "0.1.0".to_string(),
            resolved: "git+https://github.com/other/repo.git".to_string(),
            integrity: String::new(),
            installed_at: String::new(),
            targets: vec![],
        },
    );

    // Full HTTPS URL → matches the two anthropics skills
    let matched = find_packages_by_git_source(&lf, "https://github.com/anthropics/skills");
    assert_eq!(matched.len(), 2);
    assert!(matched.contains(&"skill-a".to_string()));
    assert!(matched.contains(&"skill-b".to_string()));

    // Shorthand → same result
    let matched = find_packages_by_git_source(&lf, "anthropics/skills");
    assert_eq!(matched.len(), 2);

    // Different repo → only one match
    let matched = find_packages_by_git_source(&lf, "other/repo");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0], "other-pkg");

    // No match
    let matched = find_packages_by_git_source(&lf, "nobody/nothing");
    assert!(matched.is_empty());

    // Plain registry name → not a git source, empty
    let matched = find_packages_by_git_source(&lf, "no-secrets");
    assert!(matched.is_empty());
}

#[test]
fn test_find_packages_by_git_url_with_dot_git_suffix() {
    let mut lf = Lockfile::default();
    lf.add_package(
        "my-skill".to_string(),
        LockedPackage {
            package_type: PackageType::Skill,
            version: "1.0.0".to_string(),
            resolved: "git+https://github.com/owner/repo.git".to_string(),
            integrity: String::new(),
            installed_at: String::new(),
            targets: vec![],
        },
    );

    // URL with .git suffix
    let matched = find_packages_by_git_source(&lf, "https://github.com/owner/repo.git");
    assert_eq!(matched.len(), 1);

    // URL without .git suffix (parse_git_source appends it)
    let matched = find_packages_by_git_source(&lf, "https://github.com/owner/repo");
    assert_eq!(matched.len(), 1);

    // Shorthand
    let matched = find_packages_by_git_source(&lf, "owner/repo");
    assert_eq!(matched.len(), 1);
}

// --- end-to-end: clone real repo → deploy validator → lockfile → uninstall ---

#[test]
#[serial]
fn test_e2e_deploy_local_validator_and_uninstall_by_name() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Create and deploy
    let src = work.path().join("src-val");
    make_local_validator(&src, "e2e-val", "2.0.0");
    let targets = deploy_validator("e2e-val", &src, false).unwrap();
    assert!(!targets.is_empty());

    // Write lockfile
    let mut lf = Lockfile::default();
    lf.add_package(
        "e2e-val".to_string(),
        LockedPackage {
            package_type: PackageType::Validator,
            version: "2.0.0".to_string(),
            resolved: "file:src-val".to_string(),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.clone(),
        },
    );
    lf.save(work.path()).unwrap();

    // Verify on disk
    let deployed = work.path().join(".validators/e2e-val");
    assert!(deployed.join("VALIDATOR.md").exists());

    // Lockfile has the entry
    let lf = Lockfile::load(work.path()).unwrap();
    assert!(lf.get_package("e2e-val").is_some());

    // Uninstall by name
    uninstall_validator("e2e-val", false).unwrap();
    assert!(!deployed.exists());

    // Update lockfile
    let mut lf = Lockfile::load(work.path()).unwrap();
    lf.remove_package("e2e-val");
    lf.save(work.path()).unwrap();
    let lf = Lockfile::load(work.path()).unwrap();
    assert!(lf.get_package("e2e-val").is_none());

    std::env::set_current_dir(old_dir).unwrap();
}

// --- cross-type coexistence and duplicate install tests ---

#[tokio::test]
#[serial]
async fn test_e2e_all_four_types_coexist() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // 1. Install a skill
    let skill_src = work.path().join("src-skill");
    make_local_skill(&skill_src, "test-skill", "1.0.0");
    let skill_targets = deploy_skill_to_agents("test-skill", &skill_src, None, false).unwrap();
    assert!(!skill_targets.is_empty());

    // 2. Install a validator
    let val_src = work.path().join("src-val");
    make_local_validator(&val_src, "test-val", "1.0.0");
    let val_targets = deploy_validator("test-val", &val_src, false).unwrap();
    assert!(!val_targets.is_empty());

    // 3. Install a tool
    let tool_src = work.path().join("src-tool");
    make_local_tool(&tool_src, "test-tool", "1.0.0");
    let tool_targets = deploy_tool("test-tool", &tool_src, None, false).unwrap();
    assert!(!tool_targets.is_empty());

    // 4. Install a plugin
    let plugin_src = work.path().join("src-plugin");
    make_local_plugin(&plugin_src, "test-plugin", false);
    let plugin_targets = deploy_plugin("test-plugin", &plugin_src, None, false).unwrap();
    assert!(!plugin_targets.is_empty());

    // 5. Verify all four are on disk in separate locations
    assert!(work.path().join(".skills/test-skill/SKILL.md").exists());
    assert!(work
        .path()
        .join(".validators/test-val/VALIDATOR.md")
        .exists());
    assert!(work.path().join(".tools/test-tool/TOOL.md").exists());
    assert!(work
        .path()
        .join(".claude/plugins/test-plugin/.claude-plugin/plugin.json")
        .exists());

    // 6. Verify list discovers all four
    let all = crate::list::discover_packages(false, false, false, false, None);
    let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
    assert!(
        names.contains(&"test-skill"),
        "Should find skill in list: {:?}",
        names
    );
    assert!(
        names.contains(&"test-val"),
        "Should find validator in list: {:?}",
        names
    );
    assert!(
        names.contains(&"test-tool"),
        "Should find tool in list: {:?}",
        names
    );
    assert!(
        names.contains(&"test-plugin"),
        "Should find plugin in list: {:?}",
        names
    );

    // 7. Verify type-specific filters work
    let skills_only = crate::list::discover_packages(true, false, false, false, None);
    assert!(skills_only
        .iter()
        .all(|p| p.package_type == PackageType::Skill));
    assert!(skills_only.iter().any(|p| p.name == "test-skill"));

    let tools_only = crate::list::discover_packages(false, false, true, false, None);
    assert!(tools_only
        .iter()
        .all(|p| p.package_type == PackageType::Tool));
    assert!(tools_only.iter().any(|p| p.name == "test-tool"));

    let plugins_only = crate::list::discover_packages(false, false, false, true, None);
    assert!(plugins_only
        .iter()
        .all(|p| p.package_type == PackageType::Plugin));
    assert!(plugins_only.iter().any(|p| p.name == "test-plugin"));

    let vals_only = crate::list::discover_packages(false, true, false, false, None);
    assert!(vals_only
        .iter()
        .all(|p| p.package_type == PackageType::Validator));
    assert!(vals_only.iter().any(|p| p.name == "test-val"));

    // 8. Uninstall each type independently — others remain
    uninstall_tool("test-tool", None, false).unwrap();
    assert!(!work.path().join(".tools/test-tool").exists());
    assert!(
        work.path().join(".skills/test-skill/SKILL.md").exists(),
        "Skill should survive tool uninstall"
    );
    assert!(
        work.path().join(".validators/test-val").exists(),
        "Validator should survive tool uninstall"
    );
    assert!(
        work.path().join(".claude/plugins/test-plugin").exists(),
        "Plugin should survive tool uninstall"
    );

    uninstall_plugin("test-plugin", None, false).unwrap();
    assert!(!work.path().join(".claude/plugins/test-plugin").exists());
    assert!(
        work.path().join(".skills/test-skill/SKILL.md").exists(),
        "Skill should survive plugin uninstall"
    );

    uninstall_validator("test-val", false).unwrap();
    assert!(!work.path().join(".validators/test-val").exists());
    assert!(
        work.path().join(".skills/test-skill/SKILL.md").exists(),
        "Skill should survive validator uninstall"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_tool_twice_overwrites_cleanly() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Deploy v1
    let src_v1 = work.path().join("src-v1");
    make_local_tool(&src_v1, "fs-tool", "1.0.0");
    deploy_tool("fs-tool", &src_v1, None, false).unwrap();

    let store = work.path().join(".tools/fs-tool");
    assert!(store.join("TOOL.md").exists());

    // Verify v1 is registered
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert!(mcp["mcpServers"]["fs-tool"].is_object());

    // Deploy v2 on top (same name, different version)
    let src_v2 = work.path().join("src-v2");
    make_local_tool(&src_v2, "fs-tool", "2.0.0");
    deploy_tool("fs-tool", &src_v2, None, false).unwrap();

    // Store should have v2 content
    let (_, version) = read_frontmatter(&store.join("TOOL.md")).unwrap();
    assert_eq!(version, "2.0.0", "Version should be updated to 2.0.0");

    // MCP config should still have exactly one entry for fs-tool (not duplicated)
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
            .unwrap();
    let servers = mcp["mcpServers"].as_object().unwrap();
    let fs_entries: Vec<_> = servers.keys().filter(|k| *k == "fs-tool").collect();
    assert_eq!(fs_entries.len(), 1, "Should have exactly one fs-tool entry");

    // Clean uninstall should still work
    uninstall_tool("fs-tool", None, false).unwrap();
    assert!(!store.exists());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
fn test_deploy_plugin_twice_overwrites_cleanly() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Deploy v1
    let src_v1 = work.path().join("src-v1");
    make_local_plugin(&src_v1, "my-plugin", false);
    deploy_plugin("my-plugin", &src_v1, None, false).unwrap();

    let deployed = work.path().join(".claude/plugins/my-plugin");
    assert!(deployed.join(".claude-plugin/plugin.json").exists());
    assert!(deployed.join("commands/greet.md").exists());

    // Modify v2 source to have a different command file
    let src_v2 = work.path().join("src-v2");
    make_local_plugin(&src_v2, "my-plugin", true); // now with bundled MCP
                                                   // Add an extra file to v2
    std::fs::write(src_v2.join("CHANGELOG.md"), "# Changes\nv2").unwrap();

    deploy_plugin("my-plugin", &src_v2, None, false).unwrap();

    // The deployed dir should have the v2 content
    assert!(
        deployed.join("CHANGELOG.md").exists(),
        "v2 files should be present after re-deploy"
    );
    assert!(
        deployed.join(".mcp.json").exists(),
        "v2 bundled .mcp.json should be present"
    );

    // Uninstall should still work cleanly
    uninstall_plugin("my-plugin", None, false).unwrap();
    assert!(!deployed.exists());

    std::env::set_current_dir(old_dir).unwrap();
}

// --- end-to-end: clone real repo → deploy validator → lockfile → uninstall ---

#[test]
#[serial]
fn test_e2e_clone_anthropics_deploy_validator_uninstall_by_url() {
    use crate::git_source;

    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    // Clone anthropics/skills
    let source = git_source::parse_git_source("anthropics/skills", None).unwrap();
    let clone_dir = git_source::git_clone(&source).unwrap();

    // Discover packages
    let packages = git_source::discover_packages(clone_dir.path(), None, None).unwrap();
    assert!(!packages.is_empty());

    // Pick first package, deploy it as if it were a validator (create a
    // synthetic validator from its directory to avoid needing agents)
    let pkg = &packages[0];
    let val_src = work.path().join("synthetic-val");
    make_local_validator(&val_src, &pkg.name, "1.0.0");
    deploy_validator(&pkg.name, &val_src, false).unwrap();

    // Write lockfile with git+ resolved
    let mut lf = Lockfile::default();
    lf.add_package(
        pkg.name.clone(),
        LockedPackage {
            package_type: PackageType::Validator,
            version: "1.0.0".to_string(),
            resolved: format!("git+{}", source.clone_url),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: vec![".validators/".to_string()],
        },
    );
    lf.save(work.path()).unwrap();

    // Verify deploy
    let deployed = work
        .path()
        .join(".validators")
        .join(sanitize_dir_name(&pkg.name));
    assert!(deployed.exists());

    // find_packages_by_git_source matches via URL
    let lf = Lockfile::load(work.path()).unwrap();
    let matched = find_packages_by_git_source(&lf, "https://github.com/anthropics/skills");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0], pkg.name);

    // Also matches via shorthand
    let matched = find_packages_by_git_source(&lf, "anthropics/skills");
    assert_eq!(matched.len(), 1);

    // Uninstall by name
    uninstall_validator(&pkg.name, false).unwrap();
    assert!(!deployed.exists());

    // Clean lockfile
    let mut lf = Lockfile::load(work.path()).unwrap();
    lf.remove_package(&pkg.name);
    lf.save(work.path()).unwrap();
    let lf = Lockfile::load(work.path()).unwrap();
    assert!(lf.packages.is_empty());

    std::env::set_current_dir(old_dir).unwrap();
}

// --- metadata-only tool install tests ---

#[tokio::test]
#[serial]
async fn test_install_tool_from_mcp_config_registers_server() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let mcp = crate::registry::types::McpConfig {
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-brave-search".to_string(),
        ],
        env: {
            let mut m = std::collections::BTreeMap::new();
            m.insert("BRAVE_API_KEY".to_string(), "test-key".to_string());
            m
        },
    };

    let version_detail = crate::registry::types::VersionDetail {
        name: "brave-search".to_string(),
        version: "1.0.0".to_string(),
        package_type: Some("tool".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: Some("Test tool".to_string()),
        author: None,
        license: None,
        tags: None,
        mcp: Some(mcp.clone()),
        tool_md: None,
    };

    install_tool_from_mcp_config("brave-search", &version_detail, &mcp, None, false)
        .await
        .unwrap();

    // Verify .mcp.json was created with the correct entry
    let mcp_json_path = work.path().join(".mcp.json");
    assert!(mcp_json_path.exists(), ".mcp.json should exist");
    let content = std::fs::read_to_string(&mcp_json_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let server = &json["mcpServers"]["brave-search"];
    assert_eq!(server["command"].as_str().unwrap(), "npx");
    assert_eq!(server["args"][0].as_str().unwrap(), "-y");
    assert_eq!(
        server["args"][1].as_str().unwrap(),
        "@modelcontextprotocol/server-brave-search"
    );
    assert_eq!(server["env"]["BRAVE_API_KEY"].as_str().unwrap(), "test-key");

    // Verify lockfile was updated
    let lf = Lockfile::load(work.path()).unwrap();
    let pkg = lf.get_package("brave-search").unwrap();
    assert_eq!(pkg.package_type, PackageType::Tool);
    assert_eq!(pkg.version, "1.0.0");

    std::env::set_current_dir(old_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_install_tool_from_tool_md_content() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let tool_md = r#"---
name: test-tool
description: A test tool
metadata:
  version: "2.0.0"
mcp:
  command: uvx
  args:
    - "mcp-server-test"
  transport: stdio
---

# Test Tool
"#;

    let version_detail = crate::registry::types::VersionDetail {
        name: "test-tool".to_string(),
        version: "2.0.0".to_string(),
        package_type: Some("tool".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: Some("A test tool".to_string()),
        author: None,
        license: None,
        tags: None,
        mcp: None,
        tool_md: Some(tool_md.to_string()),
    };

    install_tool_from_tool_md_content("test-tool", &version_detail, tool_md, None, false)
        .await
        .unwrap();

    // Verify .mcp.json was created with parsed TOOL.md content
    let mcp_json_path = work.path().join(".mcp.json");
    assert!(mcp_json_path.exists(), ".mcp.json should exist");
    let content = std::fs::read_to_string(&mcp_json_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let server = &json["mcpServers"]["test-tool"];
    assert_eq!(server["command"].as_str().unwrap(), "uvx");
    assert_eq!(server["args"][0].as_str().unwrap(), "mcp-server-test");

    // Verify tool was stored
    let store = work.path().join(".tools/test-tool");
    assert!(store.join("TOOL.md").exists(), "TOOL.md should be in store");

    // Verify lockfile
    let lf = Lockfile::load(work.path()).unwrap();
    let pkg = lf.get_package("test-tool").unwrap();
    assert_eq!(pkg.package_type, PackageType::Tool);
    assert_eq!(pkg.version, "2.0.0");

    std::env::set_current_dir(old_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_install_tool_from_metadata_rejects_non_tool() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let version_detail = crate::registry::types::VersionDetail {
        name: "some-skill".to_string(),
        version: "1.0.0".to_string(),
        package_type: Some("skill".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: None,
        author: None,
        license: None,
        tags: None,
        mcp: None,
        tool_md: None,
    };

    let result = install_tool_from_metadata("some-skill", &version_detail, None, false).await;
    assert!(result.is_err(), "Should reject non-tool packages");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not a tool"),
        "Error should mention not a tool: {}",
        err
    );

    std::env::set_current_dir(old_dir).unwrap();
}

/// Rejecting a non-tool package is an input-applicability failure, so it must
/// surface as `RegistryError::Validation` — `NotFound` is reserved for actual
/// registry misses.
#[tokio::test]
#[serial]
async fn test_install_tool_from_metadata_non_tool_is_validation_error() {
    let _env = IsolatedTestEnvironment::new().unwrap();
    let work = tempfile::tempdir().unwrap();
    let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(work.path()).unwrap();

    let version_detail = crate::registry::types::VersionDetail {
        name: "some-skill".to_string(),
        version: "1.0.0".to_string(),
        package_type: Some("skill".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: None,
        author: None,
        license: None,
        tags: None,
        mcp: None,
        tool_md: None,
    };

    let result = install_tool_from_metadata("some-skill", &version_detail, None, false).await;
    let err = result.unwrap_err();
    assert!(
        matches!(err, RegistryError::Validation(_)),
        "non-tool rejection must be a Validation error, got: {err:?}"
    );
}

/// Regression: registry `package_type` casing must not affect tool detection —
/// a capitalized `"Tool"` installs the same way as `"tool"`.
#[tokio::test]
#[serial]
async fn test_install_tool_from_metadata_accepts_capitalized_tool_type() {
    let _env = IsolatedTestEnvironment::new().unwrap();
    let work = tempfile::tempdir().unwrap();
    let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(work.path()).unwrap();

    let mcp = crate::registry::types::McpConfig {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@test/server".to_string()],
        env: std::collections::BTreeMap::new(),
    };

    let version_detail = crate::registry::types::VersionDetail {
        name: "cased-tool".to_string(),
        version: "1.0.0".to_string(),
        package_type: Some("Tool".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: None,
        author: None,
        license: None,
        tags: None,
        mcp: Some(mcp),
        tool_md: None,
    };

    let result = install_tool_from_metadata("cased-tool", &version_detail, None, false).await;
    assert!(
        result.is_ok(),
        "capitalized 'Tool' package_type must install as a tool: {result:?}"
    );
}

#[tokio::test]
#[serial]
async fn test_install_tool_from_mcp_config_then_uninstall() {
    let work = tempfile::tempdir().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();

    let mcp = crate::registry::types::McpConfig {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@test/server".to_string()],
        env: std::collections::BTreeMap::new(),
    };

    let version_detail = crate::registry::types::VersionDetail {
        name: "ephemeral-tool".to_string(),
        version: "1.0.0".to_string(),
        package_type: Some("tool".to_string()),
        download_url: "https://example.com/download".to_string(),
        integrity: None,
        size: None,
        published_at: "2026-01-01T00:00:00Z".to_string(),
        description: None,
        author: None,
        license: None,
        tags: None,
        mcp: Some(mcp.clone()),
        tool_md: None,
    };

    // Install
    install_tool_from_mcp_config("ephemeral-tool", &version_detail, &mcp, None, false)
        .await
        .unwrap();

    // Verify it's registered
    let mcp_json_path = work.path().join(".mcp.json");
    let content = std::fs::read_to_string(&mcp_json_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(json["mcpServers"]["ephemeral-tool"].is_object());

    // Uninstall
    uninstall_tool("ephemeral-tool", None, false).unwrap();

    // Verify it's gone from .mcp.json
    let content = std::fs::read_to_string(&mcp_json_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        json["mcpServers"]["ephemeral-tool"].is_null(),
        "Server should be removed after uninstall"
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
fn test_parse_package_type_from_string() {
    assert_eq!(
        package_type::parse_package_type("tool"),
        Some(PackageType::Tool)
    );
    assert_eq!(
        package_type::parse_package_type("skill"),
        Some(PackageType::Skill)
    );
    assert_eq!(
        package_type::parse_package_type("validator"),
        Some(PackageType::Validator)
    );
    assert_eq!(
        package_type::parse_package_type("plugin"),
        Some(PackageType::Plugin)
    );
    assert_eq!(package_type::parse_package_type("unknown"), None);
    assert_eq!(package_type::parse_package_type(""), None);
}

/// The registry may return `package_type` in any casing; parsing must accept
/// capitalized forms so the archive install path matches the metadata-only
/// path, which already compares case-insensitively.
#[test]
fn test_parse_package_type_is_case_insensitive() {
    assert_eq!(
        package_type::parse_package_type("Tool"),
        Some(PackageType::Tool)
    );
    assert_eq!(
        package_type::parse_package_type("TOOL"),
        Some(PackageType::Tool)
    );
    assert_eq!(
        package_type::parse_package_type("Skill"),
        Some(PackageType::Skill)
    );
    assert_eq!(
        package_type::parse_package_type("Validator"),
        Some(PackageType::Validator)
    );
    assert_eq!(
        package_type::parse_package_type("Plugin"),
        Some(PackageType::Plugin)
    );
    assert_eq!(
        package_type::parse_package_type("Agent"),
        Some(PackageType::Agent)
    );
}

#[test]
fn test_version_detail_deserializes_with_mcp() {
    let json = r#"{
            "name": "brave-search",
            "version": "1.0.0",
            "type": "tool",
            "description": "Web search",
            "downloadUrl": "https://example.com/download",
            "publishedAt": "2026-01-01T00:00:00Z",
            "mcp": {
                "command": "npx",
                "args": ["-y", "@mcp/server-brave"],
                "env": {"BRAVE_API_KEY": "test"}
            }
        }"#;

    let detail: crate::registry::types::VersionDetail = serde_json::from_str(json).unwrap();

    assert_eq!(detail.name, "brave-search");
    assert_eq!(detail.package_type.as_deref(), Some("tool"));
    assert!(detail.integrity.is_none());
    assert!(detail.size.is_none());

    let mcp = detail.mcp.unwrap();
    assert_eq!(mcp.command, "npx");
    assert_eq!(mcp.args, vec!["-y", "@mcp/server-brave"]);
    assert_eq!(mcp.env.get("BRAVE_API_KEY").unwrap(), "test");
}

#[test]
fn test_version_detail_deserializes_without_optional_fields() {
    let json = r#"{
            "name": "minimal",
            "version": "0.1.0",
            "downloadUrl": "https://example.com/download",
            "publishedAt": "2026-01-01T00:00:00Z"
        }"#;

    let detail: crate::registry::types::VersionDetail = serde_json::from_str(json).unwrap();

    assert_eq!(detail.name, "minimal");
    assert!(detail.package_type.is_none());
    assert!(detail.integrity.is_none());
    assert!(detail.size.is_none());
    assert!(detail.mcp.is_none());
    assert!(detail.tool_md.is_none());
}
