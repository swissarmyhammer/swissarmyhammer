//! Real-path tests for the legacy skill cleanup.
//!
//! A skill that leaves the builtin set stays in the skill store and in every
//! agent skill folder of an older install. A profile lists its retired names in
//! [`Profile::legacy_skills`]. Each test runs the real [`init_profile`] or
//! [`deinit_profile`] against a fake agent whose store and skill folders live in
//! temporary directories.

use std::path::{Path, PathBuf};

use serial_test::serial;
use swissarmyhammer_common::lifecycle::InitScope;
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

use super::*;
use crate::test_support::{assert_no_init_error, write_single_agent_config, MirdanConfigGuard};

/// The retired skill the tests seed.
const RETIRED_SKILL: &str = "shell";

/// A user skill whose name only starts like the retired skill.
const USER_SKILL: &str = "shell-notes";

/// A builtin skill the profile under test deploys.
const CURRENT_SKILL: &str = "commit";

/// The two installer entry points the tests drive.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// [`init_profile`].
    Init,
    /// [`deinit_profile`].
    Deinit,
}

/// A hermetic install environment: an isolated `$HOME`, an install root, and a
/// fake agent whose skill folder exists at both scopes.
///
/// Field order matters: the guards drop before the temporary directories they
/// point at.
struct Sandbox {
    /// Restores the agent config environment variable.
    _mirdan: MirdanConfigGuard,
    /// Restores the working directory.
    _cwd: CurrentDirGuard,
    /// Owns the isolated `$HOME`.
    env: IsolatedTestEnvironment,
    /// Owns the install root directory.
    root_dir: tempfile::TempDir,
}

impl Sandbox {
    /// Build the environment and point agent detection at the fake agent.
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let cwd = CurrentDirGuard::new(&root).unwrap();
        let config = write_single_agent_config(&root, &env.home_path());
        Self {
            _mirdan: MirdanConfigGuard::set(config),
            _cwd: cwd,
            env,
            root_dir,
        }
    }

    /// The directory that holds the store and agent folders of `scope`.
    fn base(&self, scope: InitScope) -> PathBuf {
        match scope {
            InitScope::User => self.env.home_path(),
            InitScope::Project | InitScope::Local => self.root_dir.path().canonicalize().unwrap(),
        }
    }

    /// The explicit root to pass the installer: user scope takes none.
    fn explicit_root(&self, scope: InitScope) -> Option<PathBuf> {
        match scope {
            InitScope::User => None,
            InitScope::Project | InitScope::Local => Some(self.base(scope)),
        }
    }

    /// The skill store entry for `name` at `scope`.
    fn store_entry(&self, scope: InitScope, name: &str) -> PathBuf {
        self.base(scope).join(".skills").join(name)
    }

    /// The agent skill folder entry for `name` at `scope`.
    fn agent_entry(&self, scope: InitScope, name: &str) -> PathBuf {
        self.base(scope).join(".fake/skills").join(name)
    }

    /// Seed a store entry for `name` and a symlink to it in the agent folder,
    /// the way an older install deployed it.
    fn seed_linked_skill(&self, scope: InitScope, name: &str) {
        let store = self.store_entry(scope, name);
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(
            store.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: seeded\n---\nbody\n"),
        )
        .unwrap();
        let link = self.agent_entry(scope, name);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&store, &link).unwrap();
    }

    /// Seed a real directory copy of `name` in the agent folder, the way an
    /// older project install deployed it.
    fn seed_agent_copy(&self, scope: InitScope, name: &str) {
        let copy = self.agent_entry(scope, name);
        std::fs::create_dir_all(&copy).unwrap();
        std::fs::write(copy.join("SKILL.md"), "---\nname: copy\n---\n").unwrap();
    }

    /// Run the real installer `step` for `profile` at `scope`.
    fn run(&self, step: Step, profile: &Profile, scope: InitScope) {
        let root = self.explicit_root(scope);
        let root = root.as_deref();
        let results = match step {
            Step::Init => init_profile(profile, scope, root, &NullReporter),
            Step::Deinit => deinit_profile(profile, scope, root, &NullReporter),
        };
        assert_no_init_error("legacy skill cleanup", &results);
    }
}

/// True when anything, a broken symlink included, sits at `path`.
fn present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// A profile that deploys one current skill and retires [`RETIRED_SKILL`].
fn retiring_profile() -> Profile {
    Profile {
        skills: Some(Selector::Single(CURRENT_SKILL.to_string())),
        legacy_skills: vec![RETIRED_SKILL.to_string()],
        ..Profile::default()
    }
}

/// Seed the retired skill, linked and copied, plus a user skill, then run
/// `step` and assert the retired skill is gone and the user skill stays.
fn assert_step_removes_retired_skill(step: Step, scope: InitScope) {
    let sandbox = Sandbox::new();
    sandbox.seed_linked_skill(scope, RETIRED_SKILL);
    sandbox.seed_linked_skill(scope, USER_SKILL);

    sandbox.run(step, &retiring_profile(), scope);

    assert!(
        !present(&sandbox.store_entry(scope, RETIRED_SKILL)),
        "{step:?} at {scope:?} must remove the retired skill from the store"
    );
    assert!(
        !present(&sandbox.agent_entry(scope, RETIRED_SKILL)),
        "{step:?} at {scope:?} must remove the retired skill symlink from the agent"
    );
    assert!(
        present(&sandbox.store_entry(scope, USER_SKILL)),
        "{step:?} at {scope:?} must keep the user skill in the store"
    );
    assert!(
        present(&sandbox.agent_entry(scope, USER_SKILL)),
        "{step:?} at {scope:?} must keep the user skill in the agent folder"
    );
}

#[test]
#[serial]
fn init_removes_retired_skill_at_user_scope() {
    assert_step_removes_retired_skill(Step::Init, InitScope::User);
}

#[test]
#[serial]
fn init_removes_retired_skill_at_project_scope() {
    assert_step_removes_retired_skill(Step::Init, InitScope::Project);
}

#[test]
#[serial]
fn deinit_removes_retired_skill_at_user_scope() {
    assert_step_removes_retired_skill(Step::Deinit, InitScope::User);
}

#[test]
#[serial]
fn deinit_removes_retired_skill_at_project_scope() {
    assert_step_removes_retired_skill(Step::Deinit, InitScope::Project);
}

/// A real directory copy of a retired skill in the agent folder goes too.
#[test]
#[serial]
fn init_removes_a_retired_skill_copy_from_the_agent_folder() {
    let sandbox = Sandbox::new();
    sandbox.seed_agent_copy(InitScope::Project, RETIRED_SKILL);

    sandbox.run(Step::Init, &retiring_profile(), InitScope::Project);

    assert!(!present(
        &sandbox.agent_entry(InitScope::Project, RETIRED_SKILL)
    ));
}

/// The current skills the profile deploys stay installed after init.
#[test]
#[serial]
fn init_keeps_the_current_skill_installed() {
    let sandbox = Sandbox::new();
    sandbox.seed_linked_skill(InitScope::Project, RETIRED_SKILL);

    sandbox.run(Step::Init, &retiring_profile(), InitScope::Project);

    assert!(present(
        &sandbox.store_entry(InitScope::Project, CURRENT_SKILL)
    ));
    assert!(present(
        &sandbox.agent_entry(InitScope::Project, CURRENT_SKILL)
    ));
}

/// A legacy name the profile still deploys is never removed.
#[test]
#[serial]
fn init_never_removes_a_skill_the_profile_still_deploys() {
    let sandbox = Sandbox::new();
    let profile = Profile {
        skills: Some(Selector::Single(CURRENT_SKILL.to_string())),
        legacy_skills: vec![CURRENT_SKILL.to_string()],
        ..Profile::default()
    };

    sandbox.run(Step::Init, &profile, InitScope::Project);

    assert!(present(
        &sandbox.store_entry(InitScope::Project, CURRENT_SKILL)
    ));
    assert!(present(
        &sandbox.agent_entry(InitScope::Project, CURRENT_SKILL)
    ));
}

/// A profile with no legacy skills leaves a skill named like the retired one.
#[test]
#[serial]
fn init_without_legacy_skills_keeps_every_skill() {
    let sandbox = Sandbox::new();
    sandbox.seed_linked_skill(InitScope::Project, RETIRED_SKILL);
    let profile = Profile {
        skills: Some(Selector::Single(CURRENT_SKILL.to_string())),
        ..Profile::default()
    };

    sandbox.run(Step::Init, &profile, InitScope::Project);

    assert!(present(
        &sandbox.store_entry(InitScope::Project, RETIRED_SKILL)
    ));
}

/// A retired name that nothing installed is not an error.
#[test]
#[serial]
fn init_ignores_a_retired_skill_that_is_not_installed() {
    let sandbox = Sandbox::new();

    sandbox.run(Step::Init, &retiring_profile(), InitScope::User);
}
