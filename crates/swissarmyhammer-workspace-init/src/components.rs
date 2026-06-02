//! Root-explicit `Initializable` components for workspace init.
//!
//! Both components take the workspace root as a constructor parameter and never
//! read or mutate the process working directory. This is the key difference
//! from the CWD/git-root-rooted components in `swissarmyhammer-cli`'s
//! `commands::install` module — these are safe to run from a long-lived
//! multi-board desktop process.

use std::fs;
use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::{Skill, SkillResolver};

// ── ProjectStructure (priority 20) ───────────────────────────────────

/// Creates the `.sah/` and `.prompts/` workspace directories under an explicit
/// root.
///
/// Equivalent to `swissarmyhammer-cli`'s `ProjectStructure`, but rooted at a
/// caller-supplied path via [`SwissarmyhammerDirectory::from_custom_root`]
/// instead of git-root detection / `std::env::current_dir()`. Creating the
/// directories is idempotent — `from_custom_root` and `ensure_subdir` are
/// no-ops when the layout already exists.
pub struct ProjectStructure {
    /// The workspace root; `.sah/` and `.prompts/` are created as children.
    root: PathBuf,
}

impl ProjectStructure {
    /// Create a `ProjectStructure` component rooted at `root`.
    ///
    /// `root` is the workspace directory that should *contain* `.sah/`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolve the `.sah/` managed directory for this component's root.
    ///
    /// Returns the [`SwissarmyhammerDirectory`] on success, or an error message
    /// when the directory cannot be created.
    fn sah_directory(&self) -> Result<SwissarmyhammerDirectory, String> {
        SwissarmyhammerDirectory::from_custom_root(self.root.clone())
            .map_err(|e| format!("Failed to create .sah directory: {}", e))
    }
}

impl Initializable for ProjectStructure {
    /// The component name for project structure creation.
    fn name(&self) -> &str {
        "project-structure"
    }

    /// Component category: structural setup tasks.
    fn category(&self) -> &str {
        "structure"
    }

    /// Component priority: 20 (runs before skill deployment).
    fn priority(&self) -> i32 {
        20
    }

    /// Only applicable to project and local scope initializations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Create `<root>/.sah/` (with its `workflows/` subdir) and `<root>/.prompts/`.
    ///
    /// Idempotent: re-running on an already-initialized workspace recreates
    /// nothing and reports success.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let sah_dir = match self.sah_directory() {
            Ok(d) => d,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        if let Err(e) = sah_dir.ensure_subdir("workflows") {
            return vec![InitResult::error(
                self.name(),
                format!("Failed to create workflows directory: {}", e),
            )];
        }

        let prompts_dir = self.root.join(".prompts");
        if let Err(e) = fs::create_dir_all(&prompts_dir) {
            return vec![InitResult::error(
                self.name(),
                format!("Failed to create .prompts directory: {}", e),
            )];
        }

        reporter.emit(&InitEvent::Action {
            verb: "Created".to_string(),
            message: format!("workspace structure at {}", sah_dir.root().display()),
        });

        vec![InitResult::ok(
            self.name(),
            "Workspace structure initialized",
        )]
    }
}

// ── SkillDeployment (priority 30) ────────────────────────────────────

/// The known set of init profiles a builtin skill may declare in its `profiles`
/// frontmatter list. Profile matching is an exact `==` comparison with no
/// normalization, so a typo or case-mismatch (`Kanban`, a trailing space) would
/// silently exclude a skill from every profile rather than fail. Validating
/// each builtin's `profiles` against this set turns that silent drop into a
/// loud `debug_assert!` during development. Update this set whenever a new
/// profile is introduced.
const KNOWN_PROFILES: &[&str] = &["kanban", "code-context"];

/// Deploys the builtin skills into a workspace-local `.sah/skills/` directory.
///
/// Unlike `swissarmyhammer-cli`'s `SkillDeployment` — which deploys skills into
/// *detected coding-agent* directories via mirdan's global agent detection and
/// a CWD-relative `.skills/` store — this component writes the rendered
/// `SKILL.md` files directly under `<root>/.sah/skills/<name>/`. That makes the
/// workspace self-contained: an in-process agent operating on `root` finds the
/// full skill set without any global agent configuration.
///
/// Liquid templates in skill instructions and metadata (`{% include %}`
/// partials, `{{version}}` variables) are expanded through the prompt
/// library's rendering engine, exactly as `sah init` does.
pub struct SkillDeployment {
    /// The workspace root; skills are written under `<root>/.sah/skills/`.
    root: PathBuf,
    /// Optional init-profile filter. `None` deploys every builtin skill (full
    /// workspace, as `sah init` does); `Some(p)` deploys only the skills whose
    /// `profiles` frontmatter list contains `p`.
    profile: Option<String>,
}

impl SkillDeployment {
    /// Create a `SkillDeployment` component rooted at `root` that deploys
    /// **every** builtin skill.
    ///
    /// `root` is the workspace directory that should *contain* `.sah/`.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            profile: None,
        }
    }

    /// Create a `SkillDeployment` component rooted at `root` that deploys only
    /// the builtin skills tagged with the given init `profile`.
    ///
    /// A skill belongs to a profile when its `profiles` frontmatter list
    /// contains `profile`. This is the kanban-app fast path: deploying just the
    /// `kanban`-profile cluster instead of all ~22 builtin skills.
    pub fn for_profile(root: PathBuf, profile: impl Into<String>) -> Self {
        Self {
            root,
            profile: Some(profile.into()),
        }
    }

    /// The directory that holds deployed skills: `<root>/.sah/skills/`.
    fn skills_dir(&self) -> PathBuf {
        self.root.join(".sah").join("skills")
    }

    /// True when `skill` should be deployed under this component's profile
    /// filter. With no filter every skill is deployed.
    fn matches_profile(&self, skill: &Skill) -> bool {
        match &self.profile {
            None => true,
            Some(profile) => skill.profiles.iter().any(|p| p == profile),
        }
    }
}

impl Initializable for SkillDeployment {
    /// The component name for builtin skill deployment.
    fn name(&self) -> &str {
        "skill-deployment"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 30 (runs after structure setup at priority 20).
    fn priority(&self) -> i32 {
        30
    }

    /// Only applicable to project and local scope initializations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Render and write the profile-matched builtin skills under
    /// `<root>/.sah/skills/`.
    ///
    /// Idempotent: a skill is only re-rendered and rewritten when its on-disk
    /// `SKILL.md` differs from the freshly rendered content, so repeated calls
    /// on an already-current workspace do no filesystem work.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match self.deploy_skills(reporter) {
            Ok(count) => {
                reporter.emit(&InitEvent::Action {
                    verb: "Deployed".to_string(),
                    message: format!(
                        "{} builtin skills to {}",
                        count,
                        self.skills_dir().display()
                    ),
                });
                vec![InitResult::ok(
                    self.name(),
                    format!("Deployed {} builtin skills", count),
                )]
            }
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }
}

impl SkillDeployment {
    /// Resolve, render, and write the profile-matched builtin skills into
    /// `<root>/.sah/skills/`.
    ///
    /// Returns the number of skills deployed, or an error message describing the
    /// first filesystem failure encountered. Skills already current on disk are
    /// still counted (they are "deployed"), but no rewrite occurs.
    fn deploy_skills(&self, reporter: &dyn InitReporter) -> Result<usize, String> {
        let skills_dir = self.skills_dir();
        let resolver = SkillResolver::new();
        let skills = resolver.resolve_builtins();

        let prompt_library = PromptLibrary::default();
        let template_context = version_template_context();

        fs::create_dir_all(&skills_dir)
            .map_err(|e| format!("Failed to create skills directory: {}", e))?;

        let mut count = 0;
        for (name, skill) in &skills {
            // Catch a mistagged builtin (`Kanban`, trailing space, unknown
            // profile name) loudly during development rather than letting it
            // silently fall out of every profile filter. Matching is exact
            // `==`, so an out-of-set entry would otherwise just be dropped.
            debug_assert!(
                skill
                    .profiles
                    .iter()
                    .all(|p| KNOWN_PROFILES.contains(&p.as_str())),
                "builtin skill `{name}` declares an unknown profile in {:?}; \
                 known profiles are {KNOWN_PROFILES:?} (exact match, no normalization)",
                skill.profiles
            );
            if !self.matches_profile(skill) {
                continue;
            }
            if !is_safe_skill_name(name) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("skipping unsafe skill name: {:?}", name),
                });
                continue;
            }
            let rendered = render_skill(skill, &prompt_library, &template_context, reporter);
            write_skill(&skills_dir, name, &rendered)?;
            count += 1;
        }

        Ok(count)
    }
}

/// Build a [`TemplateContext`] that exposes the crate version as `{{version}}`.
fn version_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

/// Render a skill's instructions and metadata through the Liquid engine.
///
/// Expands `{% include %}` partials and `{{version}}` variables. On a render
/// failure the raw text is kept and a warning is emitted through `reporter`,
/// matching the behavior of `sah init`.
fn render_skill(
    skill: &Skill,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
    reporter: &dyn InitReporter,
) -> Skill {
    let instructions = match prompt_library.render_text(&skill.instructions, template_context) {
        Ok(rendered) => rendered,
        Err(e) => {
            reporter.emit(&InitEvent::Warning {
                message: format!(
                    "Failed to render partials for skill '{}': {}",
                    skill.name, e
                ),
            });
            skill.instructions.clone()
        }
    };

    let mut rendered = skill.clone();
    rendered.instructions = instructions;
    for value in rendered.metadata.values_mut() {
        if value.contains("{{") {
            if let Ok(rendered_value) = prompt_library.render_text(value, template_context) {
                *value = rendered_value;
            }
        }
    }
    rendered
}

/// Write a rendered skill into `<skills_dir>/<name>/`.
///
/// Idempotent and fast on the common case: when the on-disk `SKILL.md` already
/// matches the freshly rendered content, the skill is left untouched and no
/// filesystem work is done. This is the startup-perf fix — re-opening a board
/// whose skills are already current avoids the expensive remove-and-rewrite.
///
/// When a rewrite is needed the skill directory is removed first so stale files
/// never linger. Bundled resource files are written preserving their relative
/// subdirectory structure.
fn write_skill(skills_dir: &Path, name: &str, skill: &Skill) -> Result<(), String> {
    let skill_dir = skills_dir.join(name);
    let skill_md = skill_dir.join("SKILL.md");
    let rendered_md = format_skill_md(skill);

    // Skip the rewrite when the deployed copy is already current. The currency
    // check compares ONLY the rendered SKILL.md content — it does not diff the
    // bundled resource files (`skill.resources.files`). That is sound because
    // SKILL.md embeds the crate version via `{{version}}` (see
    // `version_template_context`): any release that changes a resource file also
    // bumps the version, which changes the rendered SKILL.md and forces a
    // rewrite. Within a single version, resources are immutable, so SKILL.md
    // equality is a sufficient proxy for "the whole skill directory is current".
    if let Ok(existing) = fs::read_to_string(&skill_md) {
        if existing == rendered_md {
            return Ok(());
        }
    }

    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to clear {}: {}", skill_dir.display(), e))?;
    }
    fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create {}: {}", skill_dir.display(), e))?;

    fs::write(&skill_md, rendered_md)
        .map_err(|e| format!("Failed to write {}: {}", skill_md.display(), e))?;

    for (resource_path, content) in &skill.resources.files {
        if !is_safe_relative_path(resource_path) {
            return Err(format!("Unsafe resource path: {:?}", resource_path));
        }
        let file_path = skill_dir.join(resource_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create resource directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    Ok(())
}

/// Format a [`Skill`] as a `SKILL.md` document (YAML frontmatter + body).
///
/// Delegates the frontmatter + body assembly to
/// [`swissarmyhammer_skills::deploy::format_skill_md`], which produces output
/// that round-trips through the skill loader.
fn format_skill_md(skill: &Skill) -> String {
    swissarmyhammer_skills::deploy::format_skill_md(skill, &skill.instructions, &skill.metadata)
}

/// Validate that a skill name is a safe single path component.
///
/// Rejects empty names, path separators, and parent-directory references.
fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !Path::new(name).is_absolute()
}

/// Validate that a forward-slash-separated relative path is safe to join under
/// a skill directory.
///
/// Accepts multi-segment paths (`references/helper.md`) so skills can ship
/// progressive-disclosure content, but rejects parent-directory traversal,
/// backslashes, absolute paths, and empty segments.
fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\\') || Path::new(path).is_absolute() {
        return false;
    }
    path.split('/')
        .all(|segment| !segment.is_empty() && segment != ".." && !segment.contains(".."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::lifecycle::InitStatus;
    use swissarmyhammer_common::reporter::NullReporter;

    #[test]
    fn test_is_safe_skill_name() {
        assert!(is_safe_skill_name("plan"));
        assert!(is_safe_skill_name("code-context"));
        assert!(!is_safe_skill_name(""));
        assert!(!is_safe_skill_name("../escape"));
        assert!(!is_safe_skill_name("foo/bar"));
    }

    #[test]
    fn test_is_safe_relative_path() {
        assert!(is_safe_relative_path("helper.md"));
        assert!(is_safe_relative_path("references/helper.md"));
        assert!(!is_safe_relative_path("../escape.md"));
        assert!(!is_safe_relative_path("/abs/path.md"));
        assert!(!is_safe_relative_path(""));
    }

    #[test]
    fn test_project_structure_name_and_priority() {
        let component = ProjectStructure::new(PathBuf::from("/tmp/x"));
        assert_eq!(Initializable::name(&component), "project-structure");
        assert_eq!(component.category(), "structure");
        assert_eq!(component.priority(), 20);
    }

    #[test]
    fn test_skill_deployment_name_and_priority() {
        let component = SkillDeployment::new(PathBuf::from("/tmp/x"));
        assert_eq!(Initializable::name(&component), "skill-deployment");
        assert_eq!(component.category(), "deployment");
        assert_eq!(component.priority(), 30);
    }

    #[test]
    fn test_project_structure_creates_layout_under_explicit_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let component = ProjectStructure::new(temp.path().to_path_buf());
        let results = component.init(&InitScope::Project, &NullReporter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, InitStatus::Ok);
        assert!(temp.path().join(".sah").is_dir(), ".sah/ should exist");
        assert!(
            temp.path().join(".sah").join("workflows").is_dir(),
            ".sah/workflows/ should exist"
        );
        assert!(
            temp.path().join(".prompts").is_dir(),
            ".prompts/ should exist"
        );
    }

    #[test]
    fn test_skill_deployment_writes_builtin_skills_under_explicit_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let component = SkillDeployment::new(temp.path().to_path_buf());
        let results = component.init(&InitScope::Project, &NullReporter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, InitStatus::Ok);

        let skills_dir = temp.path().join(".sah").join("skills");
        assert!(skills_dir.is_dir(), ".sah/skills/ should exist");
        // `plan` is a known builtin skill — its SKILL.md must be present.
        let plan_md = skills_dir.join("plan").join("SKILL.md");
        assert!(plan_md.is_file(), "plan/SKILL.md should be deployed");
        let content = fs::read_to_string(&plan_md).unwrap();
        assert!(content.starts_with("---\n"), "SKILL.md has frontmatter");
        assert!(content.contains("name: plan"), "SKILL.md names the skill");
    }

    #[test]
    fn test_skill_deployment_is_idempotent() {
        let temp = tempfile::TempDir::new().unwrap();
        let component = SkillDeployment::new(temp.path().to_path_buf());

        let first = component.init(&InitScope::Project, &NullReporter);
        let second = component.init(&InitScope::Project, &NullReporter);

        assert_eq!(first[0].status, InitStatus::Ok);
        assert_eq!(second[0].status, InitStatus::Ok);
        // Re-running must not duplicate or corrupt the deployed skill.
        let plan_md = temp
            .path()
            .join(".sah")
            .join("skills")
            .join("plan")
            .join("SKILL.md");
        assert!(plan_md.is_file());
        assert_eq!(
            fs::read_to_string(&plan_md)
                .unwrap()
                .matches("name: plan")
                .count(),
            1,
            "idempotent deploy must not duplicate frontmatter"
        );
    }

    #[test]
    fn test_skill_deployment_for_profile_deploys_only_tagged_subset() {
        let temp = tempfile::TempDir::new().unwrap();
        let component = SkillDeployment::for_profile(temp.path().to_path_buf(), "kanban");
        let results = component.init(&InitScope::Project, &NullReporter);
        assert_eq!(results[0].status, InitStatus::Ok);

        let skills_dir = temp.path().join(".sah").join("skills");
        // A kanban-profile skill is deployed.
        assert!(
            skills_dir.join("kanban").join("SKILL.md").is_file(),
            "kanban-profile skill must be deployed"
        );
        assert!(
            skills_dir.join("plan").join("SKILL.md").is_file(),
            "kanban-profile skill `plan` must be deployed"
        );
        // An untagged builtin skill (`commit`) must NOT be deployed.
        assert!(
            !skills_dir.join("commit").exists(),
            "untagged `commit` skill must not be deployed under the kanban profile"
        );
    }

    #[test]
    fn test_skill_deployment_default_deploys_all_skills() {
        let temp = tempfile::TempDir::new().unwrap();
        let component = SkillDeployment::new(temp.path().to_path_buf());
        component.init(&InitScope::Project, &NullReporter);

        let skills_dir = temp.path().join(".sah").join("skills");
        // The unfiltered component deploys every builtin, including untagged ones.
        assert!(
            skills_dir.join("commit").join("SKILL.md").is_file(),
            "default (no-profile) deploy must include untagged skills like `commit`"
        );
    }

    #[test]
    fn test_write_skill_is_idempotent_skips_when_current() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{SkillName, SkillResources, SkillSource};

        let temp = tempfile::TempDir::new().unwrap();
        let skills_dir = temp.path().to_path_buf();
        let skill = Skill {
            name: SkillName::new("idem").unwrap(),
            description: "idempotent skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            profiles: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        write_skill(&skills_dir, "idem", &skill).unwrap();
        let skill_md = skills_dir.join("idem").join("SKILL.md");
        let first_mtime = fs::metadata(&skill_md).unwrap().modified().unwrap();

        // Sleep briefly so a rewrite would produce a distinct mtime.
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_skill(&skills_dir, "idem", &skill).unwrap();
        let second_mtime = fs::metadata(&skill_md).unwrap().modified().unwrap();

        assert_eq!(
            first_mtime, second_mtime,
            "re-deploying a current skill must not rewrite SKILL.md"
        );
    }

    #[test]
    fn test_components_are_skipped_for_user_scope() {
        let temp = tempfile::TempDir::new().unwrap();
        let structure = ProjectStructure::new(temp.path().to_path_buf());
        let skills = SkillDeployment::new(temp.path().to_path_buf());
        assert!(!structure.is_applicable(&InitScope::User));
        assert!(!skills.is_applicable(&InitScope::User));
        assert!(structure.is_applicable(&InitScope::Project));
        assert!(skills.is_applicable(&InitScope::Project));
    }
}
