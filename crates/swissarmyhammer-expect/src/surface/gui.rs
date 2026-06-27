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
//! [`GuiBackend`] trait, selected by `cfg` into [`ActiveBackend`]: macOS drives
//! the AX API (`AXUIElement`), Windows the UIA API (`IUIAutomation`, pressing via
//! `InvokePattern`), and Linux AT-SPI (`atspi` + `zbus`, by role+name over D-Bus).
//! Each backend reads its native tree into the shared, FFI-independent
//! [`RawAxNode`] and reuses the one [`to_a11y_node`] mapping, so the observe model
//! and the `role[name=…]` locator dialect are identical across every OS; only the
//! native role *vocabulary* differs (`AXButton` / `Button` / `push-button`, the
//! last normalized by [`atspi_role_token`] from AT-SPI's spaced `push button`). A
//! host with no supported native a11y API keeps a `cfg`-gated stub so the surface
//! still compiles everywhere. The per-OS accessibility crates are
//! target-specific dependencies, so a build pulls only its own OS's backend.
//!
//! **Testability without accessibility permission.** Driving a real native app
//! over the OS accessibility API requires a privilege the *test runner* often
//! lacks in automated/headless CI — macOS Accessibility permission, a reachable
//! Linux AT-SPI bus. The load-bearing coverage is therefore in OS-agnostic,
//! API-free unit tests that compile and run on *any* OS: the
//! [`RawAxNode`] → [`A11yNode`] mapping ([`to_a11y_node`]), the
//! [`atspi_role_token`] role normalization, the shared `role[name=…]` matcher, and
//! structural-drift-on-rename across every backend's role vocabulary — all fed a
//! plain fixture tree with no live AX/UIA/AT-SPI. The live drive/observe paths are
//! `cfg`-gated to their OS and only exercised on a CI runner for that OS; the
//! end-to-end test that launches an app is gated on [`gui_automation_available`]
//! plus a launchable fixture and skips cleanly (with a log) when either is absent.

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

/// The separator a run of whitespace in an AT-SPI role name collapses to, so the
/// role is a single token the shared `role[name=…]` grammar accepts. A hyphen is
/// inside the grammar's `[A-Za-z0-9_-]` role class.
const ATSPI_ROLE_WORD_SEPARATOR: char = '-';

/// Collapse an AT-SPI role name into a single-token role for the shared locator
/// dialect.
///
/// AT-SPI reports role names with embedded spaces (`push button`, `page tab
/// list`), but the `role[name=…]` grammar shared with the macOS/browser surfaces
/// (and [`A11ySelector`](crate::assertion::A11ySelector)) accepts a role of only
/// one `[A-Za-z][A-Za-z0-9_-]*` token. Each run of ASCII whitespace is collapsed
/// to a single [`ATSPI_ROLE_WORD_SEPARATOR`] (`push button` → `push-button`),
/// giving the Linux backend a stable single-token role vocabulary that binds and
/// drifts exactly like `AXButton` (macOS) or `Button` (Windows).
///
/// This normalizes only the *role*; a control rename still changes the accessible
/// *name*, so structural-drift semantics are unaffected. It is the Linux backend's
/// only OS-specific mapping step and, like [`to_a11y_node`], is a pure function
/// unit-tested with no live AT-SPI.
pub fn atspi_role_token(role_name: &str) -> String {
    let mut token = String::with_capacity(role_name.len());
    for word in role_name.split_whitespace() {
        if !token.is_empty() {
            token.push(ATSPI_ROLE_WORD_SEPARATOR);
        }
        token.push_str(word);
    }
    token
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
/// One implementation per platform (macOS AX, Windows UIA, Linux AT-SPI), selected
/// by `cfg` into [`ActiveBackend`], so each slots in behind the same contract
/// without touching the [`GuiAdapter`] lifecycle. The backend owns its OS-specific
/// [`Handle`](GuiBackend::Handle) (a launched process plus the native accessibility
/// handle it drives through — an AX application element, a UIA window element, or an
/// AT-SPI connection + root object reference).
pub(crate) trait GuiBackend {
    /// The provisioned, launched-app handle this backend threads from `launch`
    /// through `close`.
    type Handle;

    /// Whether the OS accessibility automation is available to *this process* right
    /// now — macOS Accessibility permission, a reachable Windows UIA client, or a
    /// reachable Linux AT-SPI bus. The integration test gates on this so a host
    /// without it skips cleanly.
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

/// The accessibility backend for the host OS: macOS drives the AX API.
#[cfg(target_os = "macos")]
pub(crate) type ActiveBackend = macos::MacBackend;

/// The accessibility backend for the host OS: Windows drives the UIA API
/// (`IUIAutomation` + `InvokePattern`).
#[cfg(target_os = "windows")]
pub(crate) type ActiveBackend = windows::WindowsBackend;

/// The accessibility backend for the host OS: Linux drives AT-SPI over D-Bus.
#[cfg(target_os = "linux")]
pub(crate) type ActiveBackend = linux::LinuxBackend;

/// The accessibility backend for hosts with no supported native a11y API: a
/// `cfg`-gated stub so the surface still compiles everywhere.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub(crate) type ActiveBackend = stub::UnsupportedBackend;

/// Whether the gui surface can drive a native app on this host right now.
///
/// `true` only on a supported OS (macOS, Windows, or Linux) **and** when the
/// process can reach that OS's accessibility automation (macOS Accessibility
/// permission, a UIA client, or the AT-SPI bus). Callers and the integration test
/// gate on this so an unsupported OS or a host without it skips cleanly rather than
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

/// The Windows accessibility backend: drives and observes a native/Tauri app
/// through UI Automation (`IUIAutomation`), pressing controls via the
/// `InvokePattern` and typing via the `ValuePattern`, with no CDP or Node. A
/// WebView2/Tauri app's web content is bridged into the same UIA tree (a
/// `<button aria-label="Go">` becomes a `Button` named `Go`), so it is reached
/// exactly like any native control. The role vocabulary is the UIA control type
/// name (`Button`, `Edit`, `Text`); the optional `--remote-debugging-port` CDP
/// escape hatch for a thin WebView2 tree is intentionally not required here.
#[cfg(target_os = "windows")]
mod windows {
    use std::process::{Child, Command};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use uiautomation::core::{UIAutomation, UIElement};
    use uiautomation::patterns::{UIInvokePattern, UIValuePattern};
    use uiautomation::types::TreeScope;

    use crate::assertion::A11ySelector;
    use crate::error::ExpectError;
    use crate::surface::a11y::{unbound, A11yAction};
    use crate::types::A11yNode;

    use super::{GuiBackend, LaunchSpec, RawAxNode, MAX_AX_DEPTH};

    /// How long to sleep between UIA-readiness polls while the launched app starts.
    const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(50);

    /// The subtree depth the readiness matcher searches under the desktop root for
    /// the launched process's top-level window (a few levels below the root).
    const WINDOW_SEARCH_DEPTH: u32 = 8;

    /// Disable the matcher's own retry loop; readiness retrying is the backend's
    /// [`wait_for_window`] poll, which honors the spec's budget.
    const MATCHER_NO_RETRY_MS: u64 = 0;

    /// The launched Windows app: the child process (killed at teardown), the UIA
    /// client, and the app's top-level window element rooted at its pid.
    pub(crate) struct WindowsApp {
        child: Child,
        automation: UIAutomation,
        window: UIElement,
    }

    /// The Windows UIA backend (zero-sized; all behavior is in the trait methods).
    pub(crate) struct WindowsBackend;

    impl GuiBackend for WindowsBackend {
        type Handle = WindowsApp;

        fn automation_available() -> bool {
            // UIA ships with Windows; constructing the COM client is the cheapest
            // honest probe that the automation API is reachable from this process.
            UIAutomation::new().is_ok()
        }

        fn launch(spec: &LaunchSpec) -> Result<WindowsApp, ExpectError> {
            let child = Command::new(&spec.executable)
                .args(&spec.args)
                .spawn()
                .map_err(ExpectError::Io)?;
            let automation = UIAutomation::new()
                .map_err(|err| ExpectError::Surface(format!("UIA init failed: {err}")))?;
            let window = wait_for_window(&automation, child.id(), spec.action_timeout)?;
            Ok(WindowsApp {
                child,
                automation,
                window,
            })
        }

        fn snapshot(handle: &WindowsApp) -> Result<RawAxNode, ExpectError> {
            Ok(read_raw(&handle.automation, &handle.window, 0))
        }

        fn perform(handle: &WindowsApp, action: &A11yAction) -> Result<(), ExpectError> {
            match action {
                A11yAction::Press { selector } => {
                    let element = find_element(&handle.automation, &handle.window, selector, 0)
                        .ok_or_else(|| unbound(selector))?;
                    press(&element)
                }
                A11yAction::Type { selector, value } => {
                    let element = find_element(&handle.automation, &handle.window, selector, 0)
                        .ok_or_else(|| unbound(selector))?;
                    set_value(&element, value)
                }
            }
        }

        fn close(handle: WindowsApp) -> Result<(), ExpectError> {
            let WindowsApp { mut child, .. } = handle;
            // Terminate the app — a `check` must not leak a running process.
            let _ = child.kill();
            let _ = child.wait();
            Ok(())
        }
    }

    /// Poll the UIA tree for the launched process's top-level window until it
    /// appears or the budget elapses.
    fn wait_for_window(
        automation: &UIAutomation,
        pid: u32,
        timeout: Duration,
    ) -> Result<UIElement, ExpectError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(root) = automation.get_root_element() {
                let matcher = automation
                    .create_matcher()
                    .from(root)
                    .process_id(pid)
                    .timeout(MATCHER_NO_RETRY_MS)
                    .depth(WINDOW_SEARCH_DEPTH);
                if let Ok(window) = matcher.find_first() {
                    return Ok(window);
                }
            }
            if Instant::now() >= deadline {
                return Err(ExpectError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }
            sleep(READINESS_POLL_INTERVAL);
        }
    }

    /// Read one live UIA element (and its subtree, up to [`MAX_AX_DEPTH`]) into a
    /// [`RawAxNode`]. The UIA control type name is the role; the UIA Name (where a
    /// bridged `aria-label` lands) is carried as the accessible name; absent
    /// attributes read as empty/`None`.
    fn read_raw(automation: &UIAutomation, element: &UIElement, depth: usize) -> RawAxNode {
        RawAxNode {
            role: control_type_name(element),
            // UIA has a single Name property (no title/description split); carry it
            // as `title` so the shared `resolve_name` picks it up.
            title: element.get_name().unwrap_or_default(),
            description: String::new(),
            value: read_value(element),
            children: read_children(automation, element, depth),
        }
    }

    /// The mapped children of `element`, empty past [`MAX_AX_DEPTH`] or when the
    /// child list cannot be read.
    fn read_children(
        automation: &UIAutomation,
        element: &UIElement,
        depth: usize,
    ) -> Vec<RawAxNode> {
        if depth >= MAX_AX_DEPTH {
            return Vec::new();
        }
        let Ok(condition) = automation.create_true_condition() else {
            return Vec::new();
        };
        match element.find_all(TreeScope::Children, &condition) {
            Ok(children) => children
                .iter()
                .map(|child| read_raw(automation, child, depth + 1))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Depth-first search for the first live element matching `selector`, by the
    /// shared `role[name=…]` predicate over its UIA control type and Name.
    fn find_element(
        automation: &UIAutomation,
        element: &UIElement,
        selector: &A11ySelector,
        depth: usize,
    ) -> Option<UIElement> {
        if selector.matches(&meta(element)) {
            return Some(element.clone());
        }
        if depth >= MAX_AX_DEPTH {
            return None;
        }
        let condition = automation.create_true_condition().ok()?;
        let children = element.find_all(TreeScope::Children, &condition).ok()?;
        children
            .iter()
            .find_map(|child| find_element(automation, child, selector, depth + 1))
    }

    /// A role+name-only [`A11yNode`] view of a live element, for the shared
    /// `role[name=…]` matcher (no children or value needed to bind a selector).
    fn meta(element: &UIElement) -> A11yNode {
        A11yNode {
            role: control_type_name(element),
            name: element.get_name().unwrap_or_default(),
            value: None,
            children: Vec::new(),
        }
    }

    /// The element's UIA control type as a role string (`Button`, `Edit`, `Text`),
    /// the gui dialect's native Windows role vocabulary; empty when unreadable.
    fn control_type_name(element: &UIElement) -> String {
        element
            .get_control_type()
            .map(|control_type| control_type.to_string())
            .unwrap_or_default()
    }

    /// The element's value via the `ValuePattern` (an input's contents), or `None`
    /// when the element does not support it or the value is empty.
    fn read_value(element: &UIElement) -> Option<String> {
        element
            .get_pattern::<UIValuePattern>()
            .ok()
            .and_then(|pattern| pattern.get_value().ok())
            .filter(|value| !value.is_empty())
    }

    /// Press an element through its UIA `InvokePattern`.
    fn press(element: &UIElement) -> Result<(), ExpectError> {
        let invoke = element.get_pattern::<UIInvokePattern>().map_err(|err| {
            ExpectError::Surface(format!("UIA element does not support InvokePattern: {err}"))
        })?;
        invoke
            .invoke()
            .map_err(|err| ExpectError::Surface(format!("UIA invoke failed: {err}")))
    }

    /// Set an element's value through its UIA `ValuePattern` (the deterministic way
    /// to type into a native field).
    fn set_value(element: &UIElement, value: &str) -> Result<(), ExpectError> {
        let pattern = element.get_pattern::<UIValuePattern>().map_err(|err| {
            ExpectError::Surface(format!("UIA element does not support ValuePattern: {err}"))
        })?;
        pattern
            .set_value(value)
            .map_err(|err| ExpectError::Surface(format!("UIA set value failed: {err}")))
    }
}

/// The Linux accessibility backend: drives and observes a native/Tauri app
/// through AT-SPI over D-Bus (`atspi` + `zbus`), pressing controls via the
/// `Action` interface and typing via the `EditableText` interface, with no CDP or
/// Node. A WebKitGTK/Tauri app's web content is bridged into the same AT-SPI tree,
/// so it is reached exactly like any native control. AT-SPI is asynchronous, so
/// the handle owns a tokio runtime the synchronous trait methods `block_on` (the
/// same pattern as the browser adapter). The role vocabulary is AT-SPI's role name
/// collapsed to a single token by [`atspi_role_token`] (`push button` →
/// `push-button`).
#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    use atspi::connection::AccessibilityConnection;
    use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
    use atspi::proxy::action::ActionProxy;
    use atspi::proxy::editable_text::EditableTextProxy;
    use atspi::proxy::text::TextProxy;
    use atspi::ObjectRefOwned;
    use futures::future::{BoxFuture, FutureExt};
    use tokio::runtime::Runtime;

    use crate::assertion::A11ySelector;
    use crate::error::ExpectError;
    use crate::surface::a11y::{unbound, A11yAction};
    use crate::types::A11yNode;

    use super::{atspi_role_token, GuiBackend, LaunchSpec, RawAxNode, MAX_AX_DEPTH};

    /// The D-Bus destination of the AT-SPI registry that owns the desktop root.
    const REGISTRY_DESTINATION: &str = "org.a11y.atspi.Registry";

    /// The object path of the AT-SPI desktop root accessible, whose children are
    /// the running application accessibles.
    const DESKTOP_ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

    /// The default action index pressed on a control (`do_action(0)` is the
    /// control's primary/default action, the AT-SPI equivalent of `AXPress`).
    const DEFAULT_ACTION_INDEX: i32 = 0;

    /// How long to sleep between AT-SPI-readiness polls while the app starts.
    const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(50);

    /// The launched Linux app: the child process (killed at teardown), the tokio
    /// runtime the AT-SPI calls run on, the AT-SPI bus connection, and the matched
    /// application accessible the snapshot/drive walk is rooted at.
    pub(crate) struct LinuxApp {
        child: Child,
        runtime: Runtime,
        connection: AccessibilityConnection,
        root: ObjectRefOwned,
    }

    /// The Linux AT-SPI backend (zero-sized; all behavior is in the trait methods).
    pub(crate) struct LinuxBackend;

    impl GuiBackend for LinuxBackend {
        type Handle = LinuxApp;

        fn automation_available() -> bool {
            // A reachable AT-SPI bus is the Linux analog of macOS's permission
            // gate; a host without the accessibility bus running connects with an
            // error and the integration test skips cleanly.
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return false;
            };
            runtime.block_on(async { AccessibilityConnection::new().await.is_ok() })
        }

        fn launch(spec: &LaunchSpec) -> Result<LinuxApp, ExpectError> {
            let child = Command::new(&spec.executable)
                .args(&spec.args)
                .spawn()
                .map_err(ExpectError::Io)?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(ExpectError::Io)?;
            let connection = runtime
                .block_on(AccessibilityConnection::new())
                .map_err(|err| to_surface_err("AT-SPI bus connect", err))?;
            let target = application_name(&spec.executable);
            let root = runtime.block_on(wait_for_application(
                &connection,
                &target,
                spec.action_timeout,
            ))?;
            Ok(LinuxApp {
                child,
                runtime,
                connection,
                root,
            })
        }

        fn snapshot(handle: &LinuxApp) -> Result<RawAxNode, ExpectError> {
            Ok(handle
                .runtime
                .block_on(read_raw(&handle.connection, handle.root.clone(), 0)))
        }

        fn perform(handle: &LinuxApp, action: &A11yAction) -> Result<(), ExpectError> {
            handle.runtime.block_on(async {
                match action {
                    A11yAction::Press { selector } => {
                        let target =
                            find_element(&handle.connection, handle.root.clone(), selector, 0)
                                .await
                                .ok_or_else(|| unbound(selector))?;
                        do_default_action(&handle.connection, &target).await
                    }
                    A11yAction::Type { selector, value } => {
                        let target =
                            find_element(&handle.connection, handle.root.clone(), selector, 0)
                                .await
                                .ok_or_else(|| unbound(selector))?;
                        set_text(&handle.connection, &target, value).await
                    }
                }
            })
        }

        fn close(handle: LinuxApp) -> Result<(), ExpectError> {
            let LinuxApp {
                mut child,
                runtime,
                connection,
                root,
            } = handle;
            // Release AT-SPI references before reaping, terminate the app, then drop
            // the runtime last (it drives the connection) — a `check` must not leak
            // a running process.
            drop(root);
            drop(connection);
            let _ = child.kill();
            let _ = child.wait();
            drop(runtime);
            Ok(())
        }
    }

    /// The accessible-name match target for the launched app: its executable file
    /// stem (e.g. `kanban-app`), which AT-SPI advertises as the application
    /// accessible's name.
    fn application_name(executable: &Path) -> String {
        executable
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Map any AT-SPI/zbus error into an [`ExpectError::Surface`] with `context`.
    fn to_surface_err(context: &str, err: impl std::fmt::Display) -> ExpectError {
        ExpectError::Surface(format!("{context}: {err}"))
    }

    /// An [`AccessibleProxy`] for `target` on the AT-SPI bus.
    async fn accessible<'a>(
        connection: &'a AccessibilityConnection,
        target: &ObjectRefOwned,
    ) -> Result<AccessibleProxy<'a>, ExpectError> {
        target
            .as_accessible_proxy(connection.connection())
            .await
            .map_err(|err| to_surface_err("AT-SPI accessible proxy", err))
    }

    /// The AT-SPI desktop root accessible, whose children are the running apps.
    async fn desktop_root(
        connection: &AccessibilityConnection,
    ) -> Result<AccessibleProxy<'_>, ExpectError> {
        AccessibleProxy::builder(connection.connection())
            .destination(REGISTRY_DESTINATION)
            .map_err(|err| to_surface_err("AT-SPI desktop destination", err))?
            .path(DESKTOP_ROOT_PATH)
            .map_err(|err| to_surface_err("AT-SPI desktop path", err))?
            .build()
            .await
            .map_err(|err| to_surface_err("AT-SPI desktop root", err))
    }

    /// The application accessible whose name matches `target` (case-insensitively),
    /// or the first application when `target` is empty; `None` when none match.
    async fn find_application(
        connection: &AccessibilityConnection,
        target: &str,
    ) -> Option<ObjectRefOwned> {
        let desktop = desktop_root(connection).await.ok()?;
        let apps = desktop.get_children().await.ok()?;
        let wanted = target.to_ascii_lowercase();
        for app in apps {
            let Ok(proxy) = accessible(connection, &app).await else {
                continue;
            };
            let Ok(name) = proxy.name().await else {
                continue;
            };
            if wanted.is_empty() || name.to_ascii_lowercase().contains(&wanted) {
                return Some(app);
            }
        }
        None
    }

    /// Poll the AT-SPI desktop for the launched app's accessible until it appears
    /// or the budget elapses.
    async fn wait_for_application(
        connection: &AccessibilityConnection,
        target: &str,
        timeout: Duration,
    ) -> Result<ObjectRefOwned, ExpectError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(app) = find_application(connection, target).await {
                return Ok(app);
            }
            if Instant::now() >= deadline {
                return Err(ExpectError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }
            tokio::time::sleep(READINESS_POLL_INTERVAL).await;
        }
    }

    /// Read one AT-SPI accessible (and its subtree, up to [`MAX_AX_DEPTH`]) into a
    /// [`RawAxNode`]. The role name is normalized to a single token by
    /// [`atspi_role_token`]; the AT-SPI name (where a bridged `aria-label` lands)
    /// is carried as the accessible name. An unreadable node maps to a default
    /// (empty) node so a partial tree never aborts the whole snapshot.
    fn read_raw<'a>(
        connection: &'a AccessibilityConnection,
        target: ObjectRefOwned,
        depth: usize,
    ) -> BoxFuture<'a, RawAxNode> {
        async move {
            let Ok(proxy) = accessible(connection, &target).await else {
                return RawAxNode::default();
            };
            RawAxNode {
                role: proxy
                    .get_role_name()
                    .await
                    .map(|role| atspi_role_token(&role))
                    .unwrap_or_default(),
                // AT-SPI has a single accessible name (no title/description split);
                // carry it as `title` so the shared `resolve_name` picks it up.
                title: proxy.name().await.unwrap_or_default(),
                description: String::new(),
                value: read_value(connection, &proxy).await,
                children: read_children(connection, &proxy, depth).await,
            }
        }
        .boxed()
    }

    /// The control's text content via the AT-SPI `Text` interface (a field's
    /// contents, a status region), or `None` when the control exposes no text
    /// interface or it is empty. This mirrors the macOS `AXValue` and Windows
    /// `ValuePattern` reads so a `role[name=…] equals <text>` assertion binds the
    /// same on Linux as on the other backends.
    async fn read_value(
        connection: &AccessibilityConnection,
        proxy: &AccessibleProxy<'_>,
    ) -> Option<String> {
        let text = TextProxy::builder(connection.connection())
            .destination(proxy.inner().destination().to_string())
            .ok()?
            .path(proxy.inner().path().to_string())
            .ok()?
            .build()
            .await
            .ok()?;
        let count = text.character_count().await.ok()?;
        let value = text.get_text(0, count).await.ok()?;
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// The mapped children of `proxy`, empty past [`MAX_AX_DEPTH`] or when the child
    /// list cannot be read.
    async fn read_children(
        connection: &AccessibilityConnection,
        proxy: &AccessibleProxy<'_>,
        depth: usize,
    ) -> Vec<RawAxNode> {
        if depth >= MAX_AX_DEPTH {
            return Vec::new();
        }
        let Ok(children) = proxy.get_children().await else {
            return Vec::new();
        };
        let mut mapped = Vec::with_capacity(children.len());
        for child in children {
            mapped.push(read_raw(connection, child, depth + 1).await);
        }
        mapped
    }

    /// Depth-first search for the first accessible matching `selector`, by the
    /// shared `role[name=…]` predicate over its normalized role and AT-SPI name.
    fn find_element<'a>(
        connection: &'a AccessibilityConnection,
        target: ObjectRefOwned,
        selector: &'a A11ySelector,
        depth: usize,
    ) -> BoxFuture<'a, Option<ObjectRefOwned>> {
        async move {
            let proxy = accessible(connection, &target).await.ok()?;
            let meta = A11yNode {
                role: proxy
                    .get_role_name()
                    .await
                    .map(|role| atspi_role_token(&role))
                    .unwrap_or_default(),
                name: proxy.name().await.unwrap_or_default(),
                value: None,
                children: Vec::new(),
            };
            if selector.matches(&meta) {
                return Some(target);
            }
            if depth >= MAX_AX_DEPTH {
                return None;
            }
            let children = proxy.get_children().await.ok()?;
            for child in children {
                if let Some(found) = find_element(connection, child, selector, depth + 1).await {
                    return Some(found);
                }
            }
            None
        }
        .boxed()
    }

    /// Press a control through its AT-SPI `Action` interface (the default action),
    /// the AT-SPI equivalent of `AXPress` / UIA `InvokePattern`.
    async fn do_default_action(
        connection: &AccessibilityConnection,
        target: &ObjectRefOwned,
    ) -> Result<(), ExpectError> {
        let proxy = accessible(connection, target).await?;
        let action = ActionProxy::builder(connection.connection())
            .destination(proxy.inner().destination().to_string())
            .map_err(|err| to_surface_err("AT-SPI action destination", err))?
            .path(proxy.inner().path().to_string())
            .map_err(|err| to_surface_err("AT-SPI action path", err))?
            .build()
            .await
            .map_err(|err| to_surface_err("AT-SPI action proxy", err))?;
        let performed = action
            .do_action(DEFAULT_ACTION_INDEX)
            .await
            .map_err(|err| to_surface_err("AT-SPI do_action", err))?;
        if performed {
            Ok(())
        } else {
            Err(ExpectError::Surface(format!(
                "AT-SPI default action {DEFAULT_ACTION_INDEX} did not perform on `{target:?}`"
            )))
        }
    }

    /// Set a control's text through its AT-SPI `EditableText` interface (the
    /// deterministic way to type into a native field).
    async fn set_text(
        connection: &AccessibilityConnection,
        target: &ObjectRefOwned,
        value: &str,
    ) -> Result<(), ExpectError> {
        let proxy = accessible(connection, target).await?;
        let editable = EditableTextProxy::builder(connection.connection())
            .destination(proxy.inner().destination().to_string())
            .map_err(|err| to_surface_err("AT-SPI editable destination", err))?
            .path(proxy.inner().path().to_string())
            .map_err(|err| to_surface_err("AT-SPI editable path", err))?
            .build()
            .await
            .map_err(|err| to_surface_err("AT-SPI editable proxy", err))?;
        let set = editable
            .set_text_contents(value)
            .await
            .map_err(|err| to_surface_err("AT-SPI set_text_contents", err))?;
        if set {
            Ok(())
        } else {
            Err(ExpectError::Surface(format!(
                "AT-SPI set_text_contents was rejected on `{target:?}`"
            )))
        }
    }
}

/// The placeholder gui backend for hosts with no supported native accessibility
/// API (anything other than macOS AX, Windows UIA, or Linux AT-SPI). The surface
/// compiles everywhere; [`automation_available`](GuiBackend::automation_available)
/// reports `false` so callers and the integration test skip cleanly rather than
/// reaching an `unimplemented!` lifecycle method.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod stub {
    use crate::error::ExpectError;
    use crate::surface::a11y::A11yAction;

    use super::{GuiBackend, LaunchSpec, RawAxNode};

    /// The message every unimplemented stub method carries.
    const UNSUPPORTED: &str =
        "the gui surface has no accessibility backend for this OS (only macOS AX, \
         Windows UIA, and Linux AT-SPI are supported)";

    /// The handle for the unsupported backend; never constructed (`launch` is
    /// `unimplemented!`).
    pub(crate) enum UnsupportedHandle {}

    /// The unsupported-OS placeholder backend.
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

    use crate::assertion::{compile, A11ySelector, AssertionOutcome};
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

    #[test]
    fn atspi_role_token_collapses_whitespace_to_a_single_token() {
        // AT-SPI reports multi-word role names; the shared `role[name=…]` grammar
        // accepts only a single `[A-Za-z][A-Za-z0-9_-]*` token, so each run of
        // whitespace collapses to one separator (`push button` → `push-button`).
        assert_eq!(atspi_role_token("push button"), "push-button");
        assert_eq!(atspi_role_token("page tab list"), "page-tab-list");
        // A single-word role is unchanged.
        assert_eq!(atspi_role_token("entry"), "entry");
        // Leading/trailing/repeated whitespace does not leak separators.
        assert_eq!(atspi_role_token("  push   button  "), "push-button");
        assert_eq!(atspi_role_token(""), "");
        // The produced token binds the shared selector grammar.
        assert_eq!(
            A11ySelector::parse_exact(&format!("{}[name=\"Go\"]", atspi_role_token("push button"))),
            Some(A11ySelector {
                role: "push-button".to_string(),
                name: Some("Go".to_string()),
            })
        );
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

    /// One OS backend's native a11y role vocabulary. The backends differ only in
    /// the role *spelling* they read into [`RawAxNode::role`] (macOS `AXButton`,
    /// Windows UIA `Button`, AT-SPI's spaced name normalized by
    /// [`atspi_role_token`] to `push-button`); the shared [`to_a11y_node`] mapping,
    /// `role[name=…]` matcher, and drift behavior are identical across all of them.
    struct RoleVocabulary {
        /// Human label for assertion messages.
        os: &'static str,
        /// The native role of a pressable control.
        button: &'static str,
        /// The native role of a text field.
        field: &'static str,
    }

    /// The role vocabulary of every gui backend, asserted against one source of
    /// truth so the cross-OS contract is one table, not parallel tests. The Linux
    /// `push-button` is exactly what [`atspi_role_token`] produces from AT-SPI's
    /// `push button`.
    const ROLE_VOCABULARIES: &[RoleVocabulary] = &[
        RoleVocabulary {
            os: "macos",
            button: "AXButton",
            field: "AXTextField",
        },
        RoleVocabulary {
            os: "windows",
            button: "Button",
            field: "Edit",
        },
        RoleVocabulary {
            os: "linux",
            button: atspi_linux_button_role(),
            field: "entry",
        },
    ];

    /// The Linux button role as the live backend would carry it: AT-SPI's
    /// `push button` run through [`atspi_role_token`], asserted here rather than
    /// hardcoded so the vocabulary table stays bound to the normalization's output.
    const fn atspi_linux_button_role() -> &'static str {
        // `atspi_role_token` is not const, so the table is checked against it at
        // runtime in `every_backend_role_vocabulary_maps_binds_and_drifts`.
        "push-button"
    }

    #[test]
    fn every_backend_role_vocabulary_maps_binds_and_drifts() {
        // The same shared mapping + `role[name=…]` dialect + drift behavior holds
        // for every OS backend's role vocabulary, fed plain fixture trees with no
        // live AX/UIA/AT-SPI — exactly what runs on this macOS box for the
        // Windows/Linux backends.
        for vocab in ROLE_VOCABULARIES {
            // The Linux row is exactly the normalization's output, not a hardcoded
            // literal that could silently drift from `atspi_role_token`.
            if vocab.os == "linux" {
                assert_eq!(
                    vocab.button,
                    atspi_role_token("push button"),
                    "the linux button role must be atspi_role_token(\"push button\")"
                );
            }

            // A window holding a named button and a text field whose value carries
            // its contents, in this OS's native role spelling.
            let raw = RawAxNode {
                role: "window".to_string(),
                children: vec![
                    titled(vocab.button, "Go"),
                    RawAxNode {
                        role: vocab.field.to_string(),
                        title: "Result".to_string(),
                        value: Some("done".to_string()),
                        ..RawAxNode::default()
                    },
                ],
                ..RawAxNode::default()
            };

            // The shared mapping carries role, name, and value through unchanged.
            let tree = to_a11y_node(&raw);
            assert_eq!(tree.children[0].role, vocab.button, "{}", vocab.os);
            assert_eq!(tree.children[0].name, "Go", "{}", vocab.os);
            assert_eq!(tree.children[1].role, vocab.field, "{}", vocab.os);
            assert_eq!(
                tree.children[1].value.as_deref(),
                Some("done"),
                "{}",
                vocab.os
            );

            // The shared `role[name=…]` dialect binds the field's value.
            let observation = observation_of(&raw);
            let assertion = compile(
                &criterion(&format!("{}[name=\"Result\"] equals done", vocab.field)),
                &observation,
            )
            .unwrap_or_else(|err| panic!("{}: compile a role[name=…] locator: {err}", vocab.os));
            assert_eq!(
                assertion.evaluate(&observation),
                AssertionOutcome::Holds,
                "{}",
                vocab.os
            );

            // Renaming the button drifts the same compiled assertion, identical to
            // the macOS/browser behavior — never a silent mis-bind.
            let button_assertion = compile(
                &criterion(&format!("{}[name=\"Go\"] equals Go", vocab.button)),
                &observation,
            )
            .unwrap_or_else(|err| panic!("{}: compile the button locator: {err}", vocab.os));
            let renamed = RawAxNode {
                role: "window".to_string(),
                children: vec![titled(vocab.button, "Start")],
                ..RawAxNode::default()
            };
            assert!(
                matches!(
                    button_assertion.evaluate(&observation_of(&renamed)),
                    AssertionOutcome::Drifted { .. }
                ),
                "{}: a renamed control must drift, not silently mis-bind",
                vocab.os
            );
        }
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
