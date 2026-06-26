//! The `cli` surface adapter — the deterministic, no-agent path.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use swissarmyhammer_project_detection::{detect_projects, ProjectType};

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::SurfaceAdapter;
use crate::types::{CliState, SurfaceState};

/// One executable command as an argv: the program followed by its arguments.
type Argv = Vec<String>;

/// The default per-run wall-clock budget when an adapter is built without an
/// explicit timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// How often [`CliAdapter::run`] polls a running child for completion before its
/// deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How far below `repo_root` project detection looks for the SUT's project type.
/// The SUT root is `repo_root` itself, so a shallow scan is enough.
const DETECT_MAX_DEPTH: usize = 1;

/// The resolved build-and-launch commands for a cli system under test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCommands {
    /// Provisioning steps, run in order during `provision`, that make the SUT
    /// runnable. May be empty.
    pub build: Vec<Argv>,
    /// The base argv that launches the SUT; `drive` appends the `When` step's
    /// own arguments to it.
    pub launch: Argv,
}

/// The provisioned cli system under test.
#[derive(Debug)]
pub struct CliSut {
    /// The directory build and run commands execute in.
    work_dir: PathBuf,
    /// The base argv that launches the SUT; `drive` appends each `When` step's
    /// arguments.
    launch: Argv,
    /// The most recent run's authoritative read; set by `drive`, read by
    /// `observe`.
    last: Option<CliState>,
}

/// The cli surface adapter: builds the SUT, runs argv, and reads
/// stdout/stderr/exit (plus any named output files).
#[derive(Debug, Clone)]
pub struct CliAdapter {
    timeout: Duration,
    output_files: Vec<String>,
}

impl CliAdapter {
    /// Create a cli adapter with the given per-run wall-clock budget.
    ///
    /// Each driven run is aborted (not allowed to hang) once it exceeds
    /// `timeout`; the abort surfaces as [`ExpectError::Timeout`].
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_expect::CliAdapter;
    /// use std::time::Duration;
    ///
    /// let adapter = CliAdapter::new(Duration::from_secs(30));
    /// # let _ = adapter;
    /// ```
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            output_files: Vec::new(),
        }
    }

    /// Also capture these files (relative to the SUT work dir) into every
    /// observation's [`CliState::files`](crate::types::CliState::files).
    pub fn capturing(mut self, output_files: impl IntoIterator<Item = String>) -> Self {
        self.output_files = output_files.into_iter().collect();
        self
    }
}

impl Default for CliAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_TIMEOUT)
    }
}

impl CliAdapter {
    /// Run `argv` in `work_dir`, capturing stdout/stderr/exit, aborting the
    /// child if it exceeds [`self.timeout`](CliAdapter::timeout) rather than
    /// blocking on it forever.
    ///
    /// stdout and stderr are drained on dedicated threads so a chatty process
    /// cannot deadlock against a full pipe buffer while the main thread polls
    /// for completion.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Surface`] for an empty `argv`,
    /// [`ExpectError::Io`] when the process cannot be spawned or waited on, and
    /// [`ExpectError::Timeout`] when the run exceeds the budget.
    fn run(&self, argv: &[String], work_dir: &Path) -> Result<CliState, ExpectError> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| ExpectError::Surface("empty command: nothing to run".to_string()))?;

        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            // Lead a new process group (pgid == child pid) so a timeout can kill
            // the whole tree, not just the direct child (see `abort_child`).
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn()?;
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let deadline = Instant::now() + self.timeout;

        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(CliState {
                    stdout: join_drain(stdout),
                    stderr: join_drain(stderr),
                    exit_code: status.code(),
                    files: BTreeMap::new(),
                });
            }
            if Instant::now() >= deadline {
                abort_child(&mut child);
                // Do not join the drain threads: even after the kill a surviving
                // descendant could keep a pipe open, and the captured output is
                // discarded on timeout anyway. Dropping detaches them so `run`
                // returns promptly rather than blocking on `read_to_string`.
                drop(stdout);
                drop(stderr);
                return Err(ExpectError::Timeout {
                    timeout_ms: self.timeout.as_millis() as u64,
                });
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Forcibly terminate a timed-out child and every descendant it spawned.
///
/// On unix the child leads its own process group (see `process_group(0)` in
/// [`CliAdapter::run`]), so signalling the group reaps grandchildren that
/// inherited the captured pipes; on other platforms only the direct child is
/// killed.
fn abort_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let group = nix::unistd::Pid::from_raw(child.id() as i32);
        let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Split a setup/launch command string into an argv on ASCII whitespace.
///
/// Setup commands are simple `program arg arg` forms; quoting and shell
/// metacharacters are not interpreted. A blank command yields an empty argv.
fn tokenize(command: &str) -> Argv {
    command.split_whitespace().map(str::to_string).collect()
}

/// Expect's own `ProjectType → {build, launch}` map for the cli surface.
///
/// `swissarmyhammer-project-detection` reports only the *type* of a project, not
/// how to build or run it, so the cli adapter owns these best-effort conventions
/// itself. `setup:` overrides them whenever a spec needs something different. The
/// match is exhaustive so a new [`ProjectType`] must be given an entry here.
fn detected_commands(project_type: ProjectType) -> CliCommands {
    let (build, launch): (&[&[&str]], &[&str]) = match project_type {
        ProjectType::Rust => (&[&["cargo", "build"]], &["cargo", "run", "--quiet", "--"]),
        ProjectType::NodeJs => (&[&["npm", "install"]], &["node", "."]),
        ProjectType::Python => (&[], &["python", "-m", "main"]),
        ProjectType::Go => (&[&["go", "build"]], &["go", "run", "."]),
        ProjectType::JavaMaven => (&[&["mvn", "-q", "package"]], &["mvn", "-q", "exec:java"]),
        ProjectType::JavaGradle => (&[&["gradle", "build"]], &["gradle", "run", "--quiet"]),
        ProjectType::CSharp => (&[&["dotnet", "build"]], &["dotnet", "run"]),
        ProjectType::CMake => (
            &[&["cmake", "--build", "."]],
            &["cmake", "--build", ".", "--target", "run"],
        ),
        ProjectType::Makefile => (&[&["make"]], &["make", "run"]),
        ProjectType::Flutter => (&[&["flutter", "build"]], &["flutter", "run"]),
        ProjectType::Php => (&[], &["php", "index.php"]),
    };
    CliCommands {
        build: build.iter().map(|argv| argv_owned(argv)).collect(),
        launch: argv_owned(launch),
    }
}

/// Copy a borrowed argv into an owned [`Argv`].
fn argv_owned(argv: &[&str]) -> Argv {
    argv.iter().map(|token| token.to_string()).collect()
}

/// Resolve the build-and-launch commands for the SUT.
///
/// When `setup` is present it overrides detection: its commands are the
/// provisioning script, where the **last** command is what launches the SUT and
/// any earlier commands are build steps. When `setup` is absent, the commands
/// come from the detected project type at `repo_root`.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] when `setup` declares no runnable command,
/// or when no project type can be detected and `setup` is absent.
fn resolve_commands(setup: Option<&Setup>, repo_root: &Path) -> Result<CliCommands, ExpectError> {
    match setup {
        Some(setup) => commands_from_setup(setup),
        None => Ok(detected_commands(detect_project_type(repo_root)?)),
    }
}

/// Build a [`CliCommands`] from a `setup:` declaration: the last command is the
/// launch, earlier ones are build steps.
fn commands_from_setup(setup: &Setup) -> Result<CliCommands, ExpectError> {
    let raw = match setup {
        Setup::Command(command) => std::slice::from_ref(command),
        Setup::Commands(commands) => commands.as_slice(),
    };
    let mut argvs: Vec<Argv> = raw
        .iter()
        .map(|command| tokenize(command))
        .filter(|argv| !argv.is_empty())
        .collect();
    let launch = argvs
        .pop()
        .ok_or_else(|| ExpectError::Surface("setup declares no runnable command".to_string()))?;
    Ok(CliCommands {
        build: argvs,
        launch,
    })
}

/// Detect the SUT's project type at `repo_root`, taking the first match.
fn detect_project_type(repo_root: &Path) -> Result<ProjectType, ExpectError> {
    let projects =
        detect_projects(repo_root, Some(DETECT_MAX_DEPTH)).map_err(ExpectError::Surface)?;
    projects
        .into_iter()
        .next()
        .map(|project| project.project_type)
        .ok_or_else(|| {
            ExpectError::Surface(format!(
                "no project type detected at {}; declare `setup:` to build and launch the SUT",
                repo_root.display()
            ))
        })
}

/// Spawn a thread that drains a child pipe to a [`String`].
fn drain<R: Read + Send + 'static>(reader: Option<R>) -> Option<JoinHandle<String>> {
    reader.map(|mut reader| {
        std::thread::spawn(move || {
            let mut buffer = String::new();
            let _ = reader.read_to_string(&mut buffer);
            buffer
        })
    })
}

/// Join a drain thread, returning what it captured (empty on any failure).
fn join_drain(handle: Option<JoinHandle<String>>) -> String {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

impl SurfaceAdapter for CliAdapter {
    type ProvisionedSut = CliSut;

    fn provision(&self, setup: Option<&Setup>, repo_root: &Path) -> Result<CliSut, ExpectError> {
        let commands = resolve_commands(setup, repo_root)?;
        for build in &commands.build {
            let outcome = self.run(build, repo_root)?;
            if outcome.exit_code != Some(0) {
                return Err(ExpectError::Surface(format!(
                    "build step `{}` failed (exit {:?}): {}",
                    build.join(" "),
                    outcome.exit_code,
                    outcome.stderr.trim()
                )));
            }
        }
        Ok(CliSut {
            work_dir: repo_root.to_path_buf(),
            launch: commands.launch,
            last: None,
        })
    }

    fn drive(&self, sut: &mut CliSut, when_step: &str) -> Result<(), ExpectError> {
        let mut argv = sut.launch.clone();
        argv.extend(tokenize(when_step));
        sut.last = Some(self.run(&argv, &sut.work_dir)?);
        Ok(())
    }

    fn observe(&self, sut: &CliSut) -> Result<SurfaceState, ExpectError> {
        let mut state = sut.last.clone().ok_or_else(|| {
            ExpectError::Surface("nothing to observe: drive the cli SUT first".to_string())
        })?;
        for name in &self.output_files {
            match std::fs::read_to_string(sut.work_dir.join(name)) {
                Ok(content) => {
                    state.files.insert(name.clone(), content);
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(ExpectError::Io(err)),
            }
        }
        Ok(SurfaceState::Cli(state))
    }

    fn teardown(&self, _sut: CliSut) -> Result<(), ExpectError> {
        // The cli SUT owns no scratch directory or long-lived process: provision
        // builds in place and drive runs each command to completion, so dropping
        // the handle is the whole teardown. Surfaces that own a scratch dir or a
        // running server release it here instead.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    /// A generous budget for runs that should finish well within it.
    const TEST_TIMEOUT: Duration = Duration::from_secs(10);

    #[cfg(unix)]
    fn write_executable(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn tokenize_splits_on_whitespace() {
        assert_eq!(
            tokenize("cargo run --quiet"),
            vec![
                "cargo".to_string(),
                "run".to_string(),
                "--quiet".to_string()
            ]
        );
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn absent_setup_falls_back_to_detected_commands() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

        let resolved = resolve_commands(None, dir.path()).expect("detect rust");
        assert_eq!(resolved, detected_commands(ProjectType::Rust));
    }

    #[test]
    fn setup_overrides_detected_commands() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

        let setup = Setup::Commands(vec![
            "cargo build --release".to_string(),
            "./target/release/app".to_string(),
        ]);
        let resolved = resolve_commands(Some(&setup), dir.path()).expect("setup resolves");

        // The last setup command is the launch; earlier ones are build steps.
        assert_eq!(
            resolved,
            CliCommands {
                build: vec![vec![
                    "cargo".to_string(),
                    "build".to_string(),
                    "--release".to_string()
                ]],
                launch: vec!["./target/release/app".to_string()],
            }
        );
        // And it is genuinely an override, not the detected default.
        assert_ne!(resolved, detected_commands(ProjectType::Rust));
    }

    #[test]
    fn single_setup_command_is_the_launch_with_no_build() {
        let dir = TempDir::new().unwrap();
        let setup = Setup::Command("./run.sh".to_string());
        let resolved = resolve_commands(Some(&setup), dir.path()).expect("single setup");
        assert_eq!(
            resolved,
            CliCommands {
                build: Vec::new(),
                launch: vec!["./run.sh".to_string()],
            }
        );
    }

    #[test]
    fn missing_project_type_without_setup_is_an_error() {
        let dir = TempDir::new().unwrap();
        let err = resolve_commands(None, dir.path()).expect_err("no marker, no setup");
        assert!(matches!(err, ExpectError::Surface(_)), "got {err:?}");
    }

    #[cfg(unix)]
    #[test]
    fn provisions_runs_argv_and_observes_stdout_and_exit() {
        let dir = TempDir::new().unwrap();
        write_executable(dir.path(), "greet.sh", "#!/bin/sh\necho \"hello $1\"\n");

        let adapter = CliAdapter::new(TEST_TIMEOUT);
        let setup = Setup::Command("./greet.sh".to_string());
        let mut sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision");
        adapter.drive(&mut sut, "world").expect("drive");

        let state = adapter.observe(&sut).expect("observe");
        match state {
            SurfaceState::Cli(cli) => {
                assert_eq!(cli.stdout, "hello world\n");
                assert_eq!(cli.exit_code, Some(0));
            }
            other => panic!("expected cli state, got {other:?}"),
        }
        adapter.teardown(sut).expect("teardown");
    }

    #[cfg(unix)]
    #[test]
    fn provision_runs_build_steps_before_launch() {
        let dir = TempDir::new().unwrap();
        // The build step creates the launchable script; the launch runs it.
        let setup = Setup::Commands(vec![
            "cp template.sh run.sh".to_string(),
            "./run.sh".to_string(),
        ]);
        write_executable(dir.path(), "template.sh", "#!/bin/sh\necho ran\n");

        let adapter = CliAdapter::new(TEST_TIMEOUT);
        let mut sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision builds");
        // The build step copied template.sh -> run.sh (still executable mode).
        adapter.drive(&mut sut, "").expect("drive");
        let state = adapter.observe(&sut).expect("observe");
        match state {
            SurfaceState::Cli(cli) => assert_eq!(cli.stdout, "ran\n"),
            other => panic!("expected cli state, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn observe_captures_named_output_files() {
        let dir = TempDir::new().unwrap();
        write_executable(
            dir.path(),
            "writer.sh",
            "#!/bin/sh\necho \"$1\" > out.txt\n",
        );

        let adapter = CliAdapter::new(TEST_TIMEOUT).capturing(["out.txt".to_string()]);
        let setup = Setup::Command("./writer.sh".to_string());
        let mut sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision");
        adapter.drive(&mut sut, "payload").expect("drive");

        let state = adapter.observe(&sut).expect("observe");
        match state {
            SurfaceState::Cli(cli) => {
                assert_eq!(
                    cli.files.get("out.txt").map(String::as_str),
                    Some("payload\n")
                );
            }
            other => panic!("expected cli state, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_exceeding_timeout_is_aborted_not_hung() {
        let dir = TempDir::new().unwrap();
        let adapter = CliAdapter::new(Duration::from_millis(50));
        let setup = Setup::Command("sleep 5".to_string());
        let mut sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision");

        let start = std::time::Instant::now();
        let err = adapter.drive(&mut sut, "").expect_err("must time out");
        assert!(
            matches!(err, ExpectError::Timeout { .. }),
            "expected timeout, got {err:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(4),
            "drive hung instead of aborting"
        );
    }

    #[cfg(unix)]
    #[test]
    fn timeout_aborts_even_when_child_spawns_a_pipe_holding_grandchild() {
        let dir = TempDir::new().unwrap();
        // The shell runs `sleep` as a child that inherits the stdout pipe; the
        // trailing `echo` stops the shell from exec-replacing itself with
        // `sleep`, so a grandchild holds the pipe open after the shell dies.
        // Killing only the direct child would leave `read_to_string` blocked on
        // that open pipe forever — this test guards against that hang.
        write_executable(dir.path(), "slow.sh", "#!/bin/sh\nsleep 8\necho done\n");

        let adapter = CliAdapter::new(Duration::from_millis(50));
        let setup = Setup::Command("./slow.sh".to_string());
        let mut sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision");

        let start = std::time::Instant::now();
        let err = adapter.drive(&mut sut, "").expect_err("must time out");
        assert!(
            matches!(err, ExpectError::Timeout { .. }),
            "expected timeout, got {err:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(4),
            "drive hung on a grandchild that kept the pipe open"
        );
    }

    #[test]
    fn observe_before_drive_is_an_error() {
        let dir = TempDir::new().unwrap();
        let adapter = CliAdapter::new(TEST_TIMEOUT);
        let setup = Setup::Command("true".to_string());
        let sut = adapter
            .provision(Some(&setup), dir.path())
            .expect("provision");
        let err = adapter.observe(&sut).expect_err("nothing driven yet");
        assert!(matches!(err, ExpectError::Surface(_)), "got {err:?}");
    }
}
