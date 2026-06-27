//! The `gui` surface adapter — drive and observe a native desktop app through
//! the OS **accessibility (AX)** tree, including a Tauri WebView's bridged web
//! content.
//!
//! Per `ideas/expect.md` §"Surface adapters" (the gui row) and §"Drilling into a
//! Tauri / Electron app": on macOS the adapter drives via the AX API
//! (`AXUIElementPerformAction` / `AXPress`) and observes by snapshotting the AX
//! subtree. A Tauri `WKWebView`'s web content is bridged into that same AX tree —
//! a `<button aria-label="Go">` appears as a named `AXButton` under an
//! `AXWebArea` — so the adapter drives it with **no CDP and no Node**. The
//! bridged tree is only as good as the web app's own semantics: a thin, role-less
//! page yields a thin AX tree, which is itself the honest testability signal.
//!
//! **Same dialect as the browser surface.** The drive dialect (`press` / `type`
//! by [`A11yAction`]) and the observe dialect (`role[name=…]` + `within` /
//! `ancestor`, resolved by the [assertion compiler](crate::assertion) against an
//! [`A11yNode`]) are *identical* to the browser surface and shared in
//! [`crate::surface::a11y`]; only the role *vocabulary* differs (native `AXButton`
//! rather than the web `button`). A genuine control rename therefore surfaces as
//! honest structural drift here exactly as it does for the browser, by
//! construction — the matcher and resolution are one implementation.
//!
//! **Per-OS seam.** The OS-specific accessibility backend sits behind the
//! [`GuiBackend`] trait, selected by `cfg` into [`ActiveBackend`]. macOS (AX) is
//! implemented here; the Windows (`IUIAutomation`) and Linux (AT-SPI) backends
//! are a separate follow-on task and are present only as `cfg`-gated
//! `unimplemented!` stubs so the surface compiles everywhere while the real
//! backends slot in behind the same trait.
//!
//! **Testability without AX permission.** Driving a real native app over AX
//! requires the *test runner* to hold macOS Accessibility permission, which is
//! frequently unavailable in automated/headless CI. The load-bearing coverage is
//! therefore in browser-free, AX-free unit tests: the
//! [`RawAxNode`] → [`A11yNode`] mapping ([`to_a11y_node`]), the shared
//! `role[name=…]` matcher, and structural-drift-on-rename — all fed a plain
//! fixture tree with no live AX. The end-to-end test that actually launches an
//! app is gated on [`gui_automation_available`] plus a launchable fixture and
//! skips cleanly (with a log) when either is absent.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::a11y::{step_resolves_mechanically, A11yAction, DEFAULT_ACTION_TIMEOUT};
use crate::surface::SurfaceAdapter;
use crate::types::{A11yNode, SurfaceState};

/// The maximum AX subtree depth read into a snapshot, a guard against pathological
/// or cyclic native trees (the bridged web content is the part of interest and is
/// shallow; native chrome above it is a handful of levels).
const MAX_AX_DEPTH: usize = 64;

/// A plain, FFI-independent accessibility node read from the OS AX tree.
///
/// This is the seam between the unsafe per-OS AX reader (which fills it from live
/// `AXUIElement`s, `IUIAutomation` elements, …) and the pure
/// [`to_a11y_node`] mapping (which turns it into the surface-neutral
/// [`A11yNode`] the locator dialect resolves against). Keeping the raw AX
/// attributes here — rather than mapping inside the FFI — is what lets the
/// load-bearing mapping logic be unit-tested with a fixture tree and **no live
/// accessibility API**.
///
/// `role` is the raw AX role (e.g. `AXButton`, `AXWebArea`, `AXTextField`); the
/// accessible name is chosen from `title` then `description` (a WebKit
/// `aria-label` is bridged into `AXDescription`), so the chosen name is computed
/// by [`to_a11y_node`], not pre-decided here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawAxNode {
    /// The raw AX role of the element (e.g. `AXButton`, `AXWebArea`).
    pub role: String,
    /// The element's `AXTitle`, or empty when it has none.
    pub title: String,
    /// The element's `AXDescription` (where a WebKit `aria-label` lands), or empty
    /// when it has none.
    pub description: String,
    /// The element's `AXValue` rendered as text (an input's contents, a status
    /// region), or `None` when it has none.
    pub value: Option<String>,
    /// The element's child nodes, in tree order.
    pub children: Vec<RawAxNode>,
}

/// Choose an accessible name from an element's `title` and `description`: the
/// `title` when it is non-empty, else the `description` (where a WebKit
/// `aria-label` is bridged). Shared by [`to_a11y_node`] and the native-AX walk so
/// "what is this element's name?" is decided in exactly one place.
pub(crate) fn resolve_name<'a>(title: &'a str, description: &'a str) -> &'a str {
    if title.is_empty() {
        description
    } else {
        title
    }
}

/// Map a raw AX subtree into the surface-neutral [`A11yNode`] tree the locator
/// dialect resolves against.
///
/// The role is carried through verbatim (the gui dialect addresses native roles
/// like `AXButton`), the accessible name is chosen by [`resolve_name`], the value
/// passes through, and children are mapped recursively. This is a pure function:
/// it is the unit-tested core of the gui surface's observe path, exercised with a
/// fixture [`RawAxNode`] and no live accessibility API.
pub fn to_a11y_node(raw: &RawAxNode) -> A11yNode {
    A11yNode {
        role: raw.role.clone(),
        name: resolve_name(&raw.title, &raw.description).to_string(),
        value: raw.value.clone(),
        children: raw.children.iter().map(to_a11y_node).collect(),
    }
}

/// How to launch the system-under-test for the gui surface: the app executable
/// (for a macOS `.app` bundle, the binary inside `Contents/MacOS/`), its
/// arguments, and the readiness budget the backend waits for the AX tree to
/// appear within.
pub(crate) struct LaunchSpec {
    /// The executable to spawn.
    pub executable: PathBuf,
    /// Arguments passed to the executable.
    pub args: Vec<String>,
    /// How long to wait for the launched app's AX tree to become readable.
    pub action_timeout: Duration,
}

/// The per-OS accessibility backend behind the gui surface.
///
/// One implementation per platform, selected by `cfg` into [`ActiveBackend`], so
/// the Windows (`IUIAutomation`) and Linux (AT-SPI) follow-on backends slot in
/// behind the same contract without touching the [`GuiAdapter`] lifecycle. The
/// backend owns its OS-specific [`Handle`](GuiBackend::Handle) (a launched process
/// plus a native AX application reference on macOS).
pub(crate) trait GuiBackend {
    /// The provisioned, launched-app handle this backend threads from `launch`
    /// through `close`.
    type Handle;

    /// Whether AX automation is available to *this process* right now — on macOS,
    /// whether the test runner holds Accessibility permission. The integration
    /// test gates on this so a host without permission skips cleanly.
    fn automation_available() -> bool;

    /// Launch the app described by `spec` and wait for its AX tree to be readable.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the executable cannot be spawned or its AX
    /// tree does not become readable within the budget.
    fn launch(spec: &LaunchSpec) -> Result<Self::Handle, ExpectError>;

    /// Snapshot the launched app's AX subtree into a [`RawAxNode`] tree.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the AX tree cannot be read.
    fn snapshot(handle: &Self::Handle) -> Result<RawAxNode, ExpectError>;

    /// Perform one parsed [`A11yAction`] (press or type) against the launched app.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the selector binds no element
    /// ([structural drift](crate::surface::a11y::unbound)) or the AX action fails.
    fn perform(handle: &Self::Handle, action: &A11yAction) -> Result<(), ExpectError>;

    /// Close the launched app, consuming the handle.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the app cannot be terminated.
    fn close(handle: Self::Handle) -> Result<(), ExpectError>;
}

/// The accessibility backend for the host OS: macOS AX here, a `cfg`-gated
/// `unimplemented!` stub elsewhere (Windows/Linux are a follow-on task).
#[cfg(target_os = "macos")]
pub(crate) type ActiveBackend = macos::MacBackend;

/// The accessibility backend for the host OS: macOS AX here, a `cfg`-gated
/// `unimplemented!` stub elsewhere (Windows/Linux are a follow-on task).
#[cfg(not(target_os = "macos"))]
pub(crate) type ActiveBackend = stub::UnsupportedBackend;

/// Whether the gui surface can drive a native app on this host right now.
///
/// `true` only on a supported OS (macOS today) **and** when the process holds the
/// OS accessibility permission. Callers and the integration test gate on this so
/// an unsupported OS or a runner without permission skips cleanly rather than
/// failing.
pub fn gui_automation_available() -> bool {
    ActiveBackend::automation_available()
}

/// The `gui` surface adapter: launches a native/Tauri app, drives it by
/// `role[name=…]` through the OS accessibility API, and snapshots its AX tree.
///
/// Construct with [`GuiAdapter::new`], passing the app executable to launch (for a
/// macOS `.app` bundle, the binary inside `Contents/MacOS/`). Driving a real app
/// needs OS accessibility permission — gate any environment-dependent use on
/// [`gui_automation_available`].
#[derive(Debug, Clone)]
pub struct GuiAdapter {
    executable: PathBuf,
    args: Vec<String>,
    action_timeout: Duration,
}

impl GuiAdapter {
    /// Create a gui adapter that launches `executable`, with no arguments and the
    /// default per-action budget.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            action_timeout: DEFAULT_ACTION_TIMEOUT,
        }
    }

    /// Set the arguments passed to the launched executable.
    pub fn with_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.args = args.into_iter().collect();
        self
    }

    /// Set the readiness/per-action budget; launching surfaces
    /// [`ExpectError::Timeout`] if the AX tree does not appear within it.
    pub fn with_action_timeout(mut self, action_timeout: Duration) -> Self {
        self.action_timeout = action_timeout;
        self
    }

    /// The [`LaunchSpec`] this adapter provisions from.
    fn launch_spec(&self) -> LaunchSpec {
        LaunchSpec {
            executable: self.executable.clone(),
            args: self.args.clone(),
            action_timeout: self.action_timeout,
        }
    }
}

/// The provisioned gui system under test: the launched app handle owned by the
/// active per-OS [`GuiBackend`], closed at teardown.
pub struct GuiSut {
    handle: <ActiveBackend as GuiBackend>::Handle,
}

impl fmt::Debug for GuiSut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuiSut").finish_non_exhaustive()
    }
}

impl SurfaceAdapter for GuiAdapter {
    type ProvisionedSut = GuiSut;

    fn provision(&self, _setup: Option<&Setup>, _repo_root: &Path) -> Result<GuiSut, ExpectError> {
        let handle = ActiveBackend::launch(&self.launch_spec())?;
        Ok(GuiSut { handle })
    }

    fn drive(&self, sut: &mut GuiSut, when_step: &str) -> Result<(), ExpectError> {
        if when_step.trim().is_empty() {
            // An empty step drives nothing (mirrors the other surfaces).
            return Ok(());
        }
        let action = A11yAction::parse(when_step).ok_or_else(|| {
            ExpectError::Surface(format!(
                "gui drive step is not a recognized action \
                 (press/type by `role[name=…]`): `{when_step}`"
            ))
        })?;
        ActiveBackend::perform(&sut.handle, &action)
    }

    fn observe(&self, sut: &GuiSut) -> Result<SurfaceState, ExpectError> {
        let raw = ActiveBackend::snapshot(&sut.handle)?;
        Ok(SurfaceState::A11y {
            tree: to_a11y_node(&raw),
        })
    }

    fn teardown(&self, sut: GuiSut) -> Result<(), ExpectError> {
        ActiveBackend::close(sut.handle)
    }

    fn resolves_mechanically(&self, when_step: &str) -> bool {
        step_resolves_mechanically(when_step)
    }
}

/// The macOS accessibility backend: drives and observes a native/Tauri app
/// through the AX API (`AXUIElement`), with no CDP or Node.
#[cfg(target_os = "macos")]
mod macos {
    use std::process::Child;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use accessibility::{AXUIElement, AXUIElementAttributes};
    use accessibility_sys::kAXPressAction;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;

    use crate::assertion::A11ySelector;
    use crate::error::ExpectError;
    use crate::surface::a11y::{unbound, A11yAction};
    use crate::types::A11yNode;

    use super::{resolve_name, GuiBackend, LaunchSpec, RawAxNode, MAX_AX_DEPTH};

    /// How long to sleep between AX-readiness polls while the launched app starts.
    const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(50);

    /// The launched macOS app: the child process (killed at teardown) and the AX
    /// application element rooted at its pid.
    pub(crate) struct MacApp {
        child: Child,
        app: AXUIElement,
    }

    /// The macOS AX backend (zero-sized; all behavior is in the trait methods).
    pub(crate) struct MacBackend;

    impl GuiBackend for MacBackend {
        type Handle = MacApp;

        fn automation_available() -> bool {
            // Safety: a parameterless C predicate with no preconditions.
            unsafe { accessibility_sys::AXIsProcessTrusted() }
        }

        fn launch(spec: &LaunchSpec) -> Result<MacApp, ExpectError> {
            let child = Command::new(&spec.executable)
                .args(&spec.args)
                .spawn()
                .map_err(ExpectError::Io)?;
            let app = AXUIElement::application(child.id() as i32);
            wait_for_ax_ready(&app, spec.action_timeout)?;
            Ok(MacApp { child, app })
        }

        fn snapshot(handle: &MacApp) -> Result<RawAxNode, ExpectError> {
            Ok(read_raw(&handle.app, 0))
        }

        fn perform(handle: &MacApp, action: &A11yAction) -> Result<(), ExpectError> {
            match action {
                A11yAction::Press { selector } => {
                    let element =
                        find_element(&handle.app, selector, 0).ok_or_else(|| unbound(selector))?;
                    press(&element)
                }
                A11yAction::Type { selector, value } => {
                    let element =
                        find_element(&handle.app, selector, 0).ok_or_else(|| unbound(selector))?;
                    set_value(&element, value)
                }
            }
        }

        fn close(handle: MacApp) -> Result<(), ExpectError> {
            let MacApp { mut child, app } = handle;
            // Release the AX reference before reaping so nothing dangles, then
            // terminate the app — a `check` must not leak a running process.
            drop(app);
            let _ = child.kill();
            let _ = child.wait();
            Ok(())
        }
    }

    /// Poll the app's AX tree until it has at least one window (it is up and
    /// readable) or the budget elapses.
    fn wait_for_ax_ready(app: &AXUIElement, timeout: Duration) -> Result<(), ExpectError> {
        let deadline = Instant::now() + timeout;
        loop {
            if app
                .windows()
                .map(|windows| !windows.is_empty())
                .unwrap_or(false)
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ExpectError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }
            sleep(READINESS_POLL_INTERVAL);
        }
    }

    /// Read one live AX element (and its subtree, up to [`MAX_AX_DEPTH`]) into a
    /// [`RawAxNode`]. Absent attributes read as empty/`None`; an unreadable child
    /// list reads as no children.
    fn read_raw(element: &AXUIElement, depth: usize) -> RawAxNode {
        RawAxNode {
            role: cfstring_attr(element.role()),
            title: cfstring_attr(element.title()),
            description: cfstring_attr(element.description()),
            value: element
                .value()
                .ok()
                .and_then(|value| cf_type_to_string(&value)),
            children: read_children(element, depth),
        }
    }

    /// The mapped children of `element`, empty past [`MAX_AX_DEPTH`] or when the
    /// child list cannot be read.
    fn read_children(element: &AXUIElement, depth: usize) -> Vec<RawAxNode> {
        if depth >= MAX_AX_DEPTH {
            return Vec::new();
        }
        match element.children() {
            Ok(children) => children
                .iter()
                .map(|child| read_raw(&child, depth + 1))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Depth-first search for the first live element matching `selector`, by the
    /// shared `role[name=…]` predicate over its AX role and accessible name.
    fn find_element(
        element: &AXUIElement,
        selector: &A11ySelector,
        depth: usize,
    ) -> Option<AXUIElement> {
        if selector.matches(&meta(element)) {
            return Some(element.clone());
        }
        if depth >= MAX_AX_DEPTH {
            return None;
        }
        let children = element.children().ok()?;
        children
            .iter()
            .find_map(|child| find_element(&child, selector, depth + 1))
    }

    /// A role+name-only [`A11yNode`] view of a live element, for the shared
    /// `role[name=…]` matcher (no children or value needed to bind a selector).
    fn meta(element: &AXUIElement) -> A11yNode {
        let title = cfstring_attr(element.title());
        let description = cfstring_attr(element.description());
        A11yNode {
            role: cfstring_attr(element.role()),
            name: resolve_name(&title, &description).to_string(),
            value: None,
            children: Vec::new(),
        }
    }

    /// Press an element through `AXUIElementPerformAction(kAXPressAction)`.
    fn press(element: &AXUIElement) -> Result<(), ExpectError> {
        element
            .perform_action(&CFString::from_static_string(kAXPressAction))
            .map_err(|err| ExpectError::Surface(format!("AX press failed: {err:?}")))
    }

    /// Set an element's `AXValue` to `value` (the deterministic way to type into a
    /// native field).
    fn set_value(element: &AXUIElement, value: &str) -> Result<(), ExpectError> {
        element
            .set_value(CFString::new(value).as_CFType())
            .map_err(|err| ExpectError::Surface(format!("AX set value failed: {err:?}")))
    }

    /// A `CFString`-valued AX attribute as an owned `String`, or empty when the
    /// attribute is absent/unreadable.
    fn cfstring_attr<E>(result: Result<CFString, E>) -> String {
        result.map(|value| value.to_string()).unwrap_or_default()
    }

    /// Render an `AXValue` ([`CFType`]) as a string when it is a string or number,
    /// else `None` (mirroring the browser surface's scalar-only value rendering).
    fn cf_type_to_string(value: &CFType) -> Option<String> {
        if let Some(text) = value.downcast::<CFString>() {
            return Some(text.to_string());
        }
        if let Some(number) = value.downcast::<CFNumber>() {
            if let Some(int) = number.to_i64() {
                return Some(int.to_string());
            }
            if let Some(float) = number.to_f64() {
                return Some(float.to_string());
            }
        }
        None
    }
}

/// The placeholder gui backend for non-macOS hosts: Windows (`IUIAutomation`) and
/// Linux (AT-SPI) are a separate follow-on task. The surface compiles everywhere;
/// the real backends slot in behind [`GuiBackend`] without touching the adapter.
#[cfg(not(target_os = "macos"))]
mod stub {
    use crate::error::ExpectError;
    use crate::surface::a11y::A11yAction;

    use super::{GuiBackend, LaunchSpec, RawAxNode};

    /// The message every unimplemented stub method carries.
    const UNSUPPORTED: &str =
        "the gui surface accessibility backend for this OS (Windows IUIAutomation / Linux AT-SPI) \
         is a separate follow-on task and is not implemented yet";

    /// The handle for the unsupported backend; never constructed (`launch` is
    /// `unimplemented!`).
    pub(crate) enum UnsupportedHandle {}

    /// The non-macOS placeholder backend.
    pub(crate) struct UnsupportedBackend;

    impl GuiBackend for UnsupportedBackend {
        type Handle = UnsupportedHandle;

        fn automation_available() -> bool {
            // No native AX backend on this OS yet, so the surface never claims it
            // can drive — callers and the integration test skip cleanly.
            false
        }

        fn launch(_spec: &LaunchSpec) -> Result<Self::Handle, ExpectError> {
            unimplemented!("{UNSUPPORTED}")
        }

        fn snapshot(_handle: &Self::Handle) -> Result<RawAxNode, ExpectError> {
            unimplemented!("{UNSUPPORTED}")
        }

        fn perform(_handle: &Self::Handle, _action: &A11yAction) -> Result<(), ExpectError> {
            unimplemented!("{UNSUPPORTED}")
        }

        fn close(_handle: Self::Handle) -> Result<(), ExpectError> {
            unimplemented!("{UNSUPPORTED}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assertion::{compile, AssertionOutcome};
    use crate::spec::Criterion;
    use crate::types::{Checkpoint, Observation, Trajectory};

    /// A leaf [`RawAxNode`] with a raw AX `role` and `title`.
    fn titled(role: &str, title: &str) -> RawAxNode {
        RawAxNode {
            role: role.to_string(),
            title: title.to_string(),
            ..RawAxNode::default()
        }
    }

    #[test]
    fn maps_role_name_and_value_through_the_raw_tree() {
        // A native window holding a button (named by AXTitle) and a text field
        // whose AXValue carries its contents.
        let raw = RawAxNode {
            role: "AXWindow".to_string(),
            title: "Main".to_string(),
            children: vec![
                titled("AXButton", "Go"),
                RawAxNode {
                    role: "AXTextField".to_string(),
                    title: "Result".to_string(),
                    value: Some("done".to_string()),
                    ..RawAxNode::default()
                },
            ],
            ..RawAxNode::default()
        };

        let tree = to_a11y_node(&raw);
        assert_eq!(tree.role, "AXWindow");
        assert_eq!(tree.name, "Main");
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].role, "AXButton");
        assert_eq!(tree.children[0].name, "Go");
        assert_eq!(tree.children[0].value, None);
        assert_eq!(tree.children[1].role, "AXTextField");
        assert_eq!(tree.children[1].name, "Result");
        assert_eq!(tree.children[1].value.as_deref(), Some("done"));
    }

    #[test]
    fn resolves_aria_label_as_the_name_for_bridged_web_content() {
        // The Tauri case: a `<button aria-label="Save">` is bridged into an
        // AXButton under an AXWebArea with an empty AXTitle and the label in
        // AXDescription. The accessible name must come from the description.
        let raw = RawAxNode {
            role: "AXWebArea".to_string(),
            children: vec![RawAxNode {
                role: "AXButton".to_string(),
                description: "Save".to_string(),
                ..RawAxNode::default()
            }],
            ..RawAxNode::default()
        };

        let tree = to_a11y_node(&raw);
        assert_eq!(tree.role, "AXWebArea");
        assert_eq!(tree.children[0].role, "AXButton");
        assert_eq!(
            tree.children[0].name, "Save",
            "an empty AXTitle falls back to the AXDescription (aria-label)"
        );
    }

    #[test]
    fn resolve_name_prefers_title_then_description() {
        assert_eq!(resolve_name("Title", "Desc"), "Title");
        assert_eq!(resolve_name("", "Desc"), "Desc");
        assert_eq!(resolve_name("", ""), "");
    }

    /// Wrap a mapped tree in a single-checkpoint observation so the a11y locator
    /// dialect can be compiled and evaluated against it.
    fn observation_of(raw: &RawAxNode) -> Observation {
        Observation {
            path: "fixture".to_string(),
            checkpoints: vec![Checkpoint {
                after: "final".to_string(),
                state: SurfaceState::A11y {
                    tree: to_a11y_node(raw),
                },
                duration: Duration::from_millis(1),
            }],
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// An unchecked criterion from `text`.
    fn criterion(text: &str) -> Criterion {
        Criterion {
            text: text.to_string(),
            checked: false,
        }
    }

    #[test]
    fn binds_a_role_name_locator_over_the_mapped_native_tree() {
        // The same `role[name=…]` dialect as the browser surface binds against the
        // mapped *native* tree: a text field's observed value resolves and the
        // assertion holds.
        let raw = RawAxNode {
            role: "AXWindow".to_string(),
            children: vec![RawAxNode {
                role: "AXTextField".to_string(),
                title: "Result".to_string(),
                value: Some("done".to_string()),
                ..RawAxNode::default()
            }],
            ..RawAxNode::default()
        };
        let observation = observation_of(&raw);

        let assertion = compile(
            &criterion("AXTextField[name=\"Result\"] equals done"),
            &observation,
        )
        .expect("compile an AX role[name=…] locator");
        assert_eq!(assertion.evaluate(&observation), AssertionOutcome::Holds);
    }

    #[test]
    fn a_renamed_native_control_surfaces_as_structural_drift() {
        // Compile a locator that binds against the original tree, then rename the
        // control: the *same* compiled assertion no longer binds and surfaces as
        // structural drift, identical to the browser surface's behavior.
        let original = RawAxNode {
            role: "AXWindow".to_string(),
            children: vec![titled("AXButton", "Go")],
            ..RawAxNode::default()
        };
        let assertion = compile(
            &criterion("AXButton[name=\"Go\"] equals Go"),
            &observation_of(&original),
        )
        .expect("compile against the original name");

        let renamed = RawAxNode {
            role: "AXWindow".to_string(),
            children: vec![titled("AXButton", "Start")],
            ..RawAxNode::default()
        };
        assert!(
            matches!(
                assertion.evaluate(&observation_of(&renamed)),
                AssertionOutcome::Drifted { .. }
            ),
            "a renamed control must drift, not silently mis-bind"
        );
    }

    #[test]
    fn resolves_mechanically_delegates_to_the_shared_a11y_dialect() {
        // The gui adapter reuses the shared press/type dialect over native AX
        // roles, and routes an unparseable step to the agent fallback.
        let adapter = GuiAdapter::new("/does/not/launch");
        assert!(adapter.resolves_mechanically("press AXButton[name=\"Go\"]"));
        assert!(adapter.resolves_mechanically("   "));
        assert!(!adapter.resolves_mechanically("do something clever"));
    }

    #[test]
    fn gui_automation_available_is_callable_without_panicking() {
        // A smoke check of the per-OS gate: it must answer without launching
        // anything (false off-platform; whatever permission state on macOS).
        let _ = gui_automation_available();
    }
}
