//! The shared install lifecycle for single-server tool CLIs.
//!
//! `kanban` and `code-context` install the same shape: one
//! `<name> serve` MCP server, a selection of builtin skills, and a small set of
//! genuine tool-lifecycle components (a `.code-context/` directory, the
//! `.kanban/` git merge drivers). Neither varies that shape by
//! [`InitScope`].
//!
//! Each CLI implements [`ToolInstall`] on a marker type in its registry module
//! and states only what differs — its server name, its skill selection, and its
//! components. Building the profile, building the component registry, and
//! sequencing install/uninstall happen here once instead of once per CLI.
//!
//! `sah` is not a tool CLI: it also installs agents, validators, the statusline,
//! and the edit redirect, so it builds its own
//! [`Profile`](crate::install::Profile) directly.

use std::path::Path;

use clap::ArgMatches;
use swissarmyhammer_cli_completions::lifecycle;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{CliReporter, InitReporter};

use crate::install::{deinit_profile_with_registry, init_profile_with_registry, Profile, Selector};

/// The per-CLI facts that distinguish one tool CLI's install lifecycle.
///
/// The three required items are the only things that differ between tool CLIs.
/// Everything built from them — the [`Profile`], the [`InitRegistry`], and the
/// install/uninstall sequencing — is provided here, so a new tool CLI declares
/// data rather than restating the lifecycle.
pub trait ToolInstall {
    /// The MCP server name registered under each agent's config. It is also the
    /// binary name and the server identity the CLI's `serve` command advertises.
    const SERVER_NAME: &'static str;

    /// The builtin skills this tool deploys.
    ///
    /// Selection does not vary by [`InitScope`]: a user-scope install lands the
    /// same skills in the global store that a project-scope install lands in the
    /// project store.
    fn skills() -> Selector;

    /// Register the tool's genuine lifecycle components into `registry`.
    ///
    /// These are the concerns that are not expressible as profile data — a
    /// project directory, a git merge driver, a tool denial — and they are the
    /// only per-CLI install code left outside the profile installer.
    fn register_components(registry: &mut InitRegistry);

    /// The install profile `<tool> init` / `<tool> deinit` apply.
    fn profile() -> Profile {
        Profile::tool(Self::SERVER_NAME, Self::skills())
    }

    /// A registry preloaded with this tool's lifecycle components.
    fn component_registry() -> InitRegistry {
        let mut registry = InitRegistry::new();
        Self::register_components(&mut registry);
        registry
    }

    /// Install this tool at `scope`, returning every step's result.
    ///
    /// The profile is applied first, then the tool-lifecycle components — see
    /// [`init_profile_with_registry`]. `root` roots the install at an explicit
    /// directory instead of the process working directory.
    fn init(scope: InitScope, root: Option<&Path>, reporter: &dyn InitReporter) -> Vec<InitResult> {
        init_profile_with_registry(
            &Self::profile(),
            &Self::component_registry(),
            scope,
            root,
            reporter,
        )
    }

    /// Remove this tool at `scope`, returning every step's result.
    ///
    /// The mirror of [`ToolInstall::init`]: the tool-lifecycle components are
    /// deinitialized first, then the profile is uninstalled — see
    /// [`deinit_profile_with_registry`].
    fn deinit(
        scope: InitScope,
        root: Option<&Path>,
        reporter: &dyn InitReporter,
    ) -> Vec<InitResult> {
        deinit_profile_with_registry(
            &Self::profile(),
            &Self::component_registry(),
            scope,
            root,
            reporter,
        )
    }
}

/// Which half of a tool CLI's install lifecycle to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// `<tool> init` — apply the profile, then initialize the components.
    Install,
    /// `<tool> deinit` — deinitialize the components, then remove the profile.
    Uninstall,
}

/// Run one half of `T`'s install lifecycle and return the process exit code.
///
/// This is the whole body of every tool CLI's `init` and `deinit` subcommand:
/// pick the direction, run it, and collapse the per-step results into the
/// shared exit-code contract — 0 when every step succeeded, 1 when any step
/// reported [`InitStatus::Error`].
pub fn run_lifecycle<T: ToolInstall>(
    lifecycle: Lifecycle,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> i32 {
    let results = match lifecycle {
        Lifecycle::Install => T::init(scope, root, reporter),
        Lifecycle::Uninstall => T::deinit(scope, root, reporter),
    };
    i32::from(results.iter().any(|r| r.status == InitStatus::Error))
}

/// Run `T`'s `init` or `deinit` subcommand from its parsed clap matches and
/// return the process exit code.
///
/// This is the entire body of those two subcommands for every tool CLI: read
/// the target scope off `matches`, root the run at the process working
/// directory (the CLI contract), and report progress through [`CliReporter`].
pub fn run_lifecycle_command<T: ToolInstall>(direction: Lifecycle, matches: &ArgMatches) -> i32 {
    run_lifecycle::<T>(
        direction,
        lifecycle::target_scope(matches),
        None,
        &CliReporter,
    )
}

/// Declare a tool CLI's install identity: the marker type and its
/// [`ToolInstall`] impl.
///
/// Every tool CLI states the same three facts — the MCP server name, the skill
/// selection, and the genuine lifecycle components — so the type, its derives,
/// and the impl scaffolding are generated here instead of restated per CLI.
/// The facts themselves stay in the CLI's own registry module.
///
/// ```ignore
/// mirdan::declare_tool_install! {
///     /// kanban's install identity, applied by `kanban init` / `kanban deinit`.
///     KanbanInstall {
///         server: "kanban",
///         skills: Selector::All,
///         components: [KanbanTool::new()],
///     }
/// }
/// ```
#[macro_export]
macro_rules! declare_tool_install {
    (
        $(#[$doc:meta])*
        $name:ident {
            server: $server:expr,
            skills: $skills:expr,
            components: [$($component:expr),+ $(,)?] $(,)?
        }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $crate::tool_install::ToolInstall for $name {
            const SERVER_NAME: &'static str = $server;

            fn skills() -> $crate::install::Selector {
                $skills
            }

            fn register_components(
                registry: &mut ::swissarmyhammer_common::lifecycle::InitRegistry,
            ) {
                $(registry.register($component);)+
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::lifecycle::Initializable;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    use std::collections::BTreeSet;
    use swissarmyhammer_skills::SkillResolver;

    use crate::test_support::{
        assert_no_init_error, assert_tool_component_count, assert_tool_lifecycle_round_trip,
        assert_tool_profile, write_single_agent_config, MirdanConfigGuard,
    };

    /// The builtin skill [`FakeTool`] deploys.
    const FAKE_SKILLS: &[&str] = &["task"];

    /// A builtin skill that was removed. No install deploys it.
    const REMOVED_SKILL: &str = "shell";

    /// A stand-in tool CLI: one MCP server, one skill, one lifecycle component.
    struct FakeTool;

    /// The genuine tool-lifecycle component [`FakeTool`] registers.
    struct FakeComponent;

    impl Initializable for FakeComponent {
        fn name(&self) -> &str {
            "fake-component"
        }

        fn category(&self) -> &str {
            "structure"
        }
    }

    impl ToolInstall for FakeTool {
        const SERVER_NAME: &'static str = "faketool";

        fn skills() -> Selector {
            Selector::Single("task".to_string())
        }

        fn register_components(registry: &mut InitRegistry) {
            registry.register(FakeComponent);
        }
    }

    /// A stand-in tool CLI that deploys every builtin skill.
    struct FakeAllSkillsTool;

    impl ToolInstall for FakeAllSkillsTool {
        const SERVER_NAME: &'static str = "faketool";

        fn skills() -> Selector {
            Selector::All
        }

        fn register_components(_registry: &mut InitRegistry) {}
    }

    /// The provided `profile()` builds the tool shape: the `<name> serve` MCP
    /// server and the declared skills, and nothing sah-only.
    #[test]
    fn profile_declares_the_server_and_skills_only() {
        assert_tool_profile::<FakeTool>(&Selector::Single("task".to_string()));
    }

    /// An install of every builtin skill deploys exactly the builtin set, and
    /// the removed `shell` skill is not part of it.
    #[test]
    #[serial_test::serial(cwd)]
    fn all_skills_install_deploys_the_builtin_set_without_the_removed_skill() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let config_path = write_single_agent_config(&root, &env.home_path());
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results =
            FakeAllSkillsTool::init(InitScope::Project, Some(root.as_path()), &NullReporter);
        assert_no_init_error("all skills init", &results);

        let deployed: BTreeSet<String> = std::fs::read_dir(root.join(".fake/skills"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let builtin: BTreeSet<String> = SkillResolver::new()
            .resolve_builtins()
            .into_keys()
            .collect();
        assert!(!deployed.contains(REMOVED_SKILL));
        assert_eq!(deployed, builtin);
    }

    /// The provided `component_registry()` returns a registry holding exactly
    /// the components the implementor registered.
    #[test]
    fn component_registry_holds_the_registered_components() {
        assert_tool_component_count::<FakeTool>(1);
    }

    /// The provided `deinit()` is the mirror of `init()`: the skill the install
    /// deployed and the MCP registration it wrote are gone afterward.
    #[test]
    #[serial_test::serial(cwd)]
    fn deinit_reverses_init() {
        assert_tool_lifecycle_round_trip::<FakeTool>(InitScope::Project, FAKE_SKILLS);
    }

    /// Both directions of `run_lifecycle` report 0 when every step succeeds.
    #[test]
    #[serial_test::serial(cwd)]
    fn run_lifecycle_reports_success_as_exit_code_zero() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let config_path = write_single_agent_config(&root, &env.home_path());
        let _mirdan = MirdanConfigGuard::set(&config_path);

        for direction in [Lifecycle::Install, Lifecycle::Uninstall] {
            assert_eq!(
                run_lifecycle::<FakeTool>(
                    direction,
                    InitScope::Project,
                    Some(&root),
                    &NullReporter
                ),
                0,
                "{direction:?} must exit 0 when every step succeeds"
            );
        }
    }

    /// Both directions of `run_lifecycle` report 1 when any step errors.
    #[test]
    #[serial_test::serial(cwd)]
    fn run_lifecycle_reports_failure_as_exit_code_one() {
        // Agent detection is pointed at a config that is not there, so every
        // profile step errors — the failure path both directions must surface.
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let _mirdan = MirdanConfigGuard::set(Path::new("/nonexistent/agents.yaml"));

        for direction in [Lifecycle::Install, Lifecycle::Uninstall] {
            assert_eq!(
                run_lifecycle::<FakeTool>(
                    direction,
                    InitScope::Project,
                    Some(&root),
                    &NullReporter
                ),
                1,
                "{direction:?} must exit 1 when a step errors"
            );
        }
    }
}
