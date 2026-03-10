//! Lifecycle management framework for SwissArmyHammer components
//!
//! Parallel to [`crate::health`] (`Doctorable`), this module provides an `Initializable`
//! trait for components that need project setup/teardown and runtime start/stop.
//!
//! Four lifecycle operations, all explicit — nothing fires automatically:
//! - `init` / `deinit` — one-time project setup/teardown (`sah init` / `sah deinit`)
//! - `start` / `stop` — runtime background work (indexing, LSP, watchers)

/// Scope for init/deinit operations — mirrors CLI install targets without coupling to clap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitScope {
    /// Project-level: `.swissarmyhammer/`, `.skills/`, `.agents/`, etc.
    Project,
    /// Local user scope: `~/.claude.json` MCP registration
    Local,
    /// User-wide scope: global config
    User,
}

/// Status of an init/deinit/start/stop operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStatus {
    Ok,
    Warning,
    Error,
    Skipped,
}

/// Result of a single lifecycle operation.
#[derive(Debug, Clone)]
pub struct InitResult {
    pub name: String,
    pub status: InitStatus,
    pub message: String,
}

impl InitResult {
    pub fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), status: InitStatus::Ok, message: message.into() }
    }

    pub fn warning(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), status: InitStatus::Warning, message: message.into() }
    }

    pub fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), status: InitStatus::Error, message: message.into() }
    }

    pub fn skipped(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), status: InitStatus::Skipped, message: message.into() }
    }
}

/// Trait for components that declare lifecycle operations.
///
/// The caller decides when to invoke — nothing fires automatically.
/// All methods have default empty impls so components opt in to what they need.
///
/// - `init`/`deinit` take scope (project setup varies by target)
/// - `start`/`stop` have no scope (runtime is runtime)
/// - Sync, not async — init operations are filesystem work. `start()` can spawn
///   tokio tasks internally if needed.
pub trait Initializable {
    /// Human-readable name of this component.
    fn name(&self) -> &str;

    /// Category (e.g., "tools", "system", "configuration").
    fn category(&self) -> &str;

    /// Priority for ordering — lower runs first. Default 0.
    fn priority(&self) -> i32 { 0 }

    /// Whether this component should participate in lifecycle operations.
    fn is_applicable(&self, _scope: &InitScope) -> bool { true }

    /// One-time project setup. Called by `sah init`.
    fn init(&self, _scope: &InitScope) -> Vec<InitResult> { vec![] }

    /// One-time project teardown. Called by `sah deinit`.
    fn deinit(&self, _scope: &InitScope) -> Vec<InitResult> { vec![] }

    /// Start runtime background work. Called explicitly by serve/connect.
    fn start(&self) -> Vec<InitResult> { vec![] }

    /// Stop runtime background work. Called on shutdown.
    fn stop(&self) -> Vec<InitResult> { vec![] }
}

/// Registry that collects `Initializable` components and runs lifecycle operations
/// in priority order.
pub struct InitRegistry {
    components: Vec<Box<dyn Initializable>>,
}

impl InitRegistry {
    pub fn new() -> Self {
        Self { components: Vec::new() }
    }

    pub fn register<T: Initializable + 'static>(&mut self, component: T) {
        self.components.push(Box::new(component));
    }

    /// Sort components by priority (stable sort preserves registration order for ties).
    fn sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.components.len()).collect();
        indices.sort_by_key(|&i| self.components[i].priority());
        indices
    }

    pub fn run_all_init(&self, scope: &InitScope) -> Vec<InitResult> {
        self.sorted_indices()
            .into_iter()
            .flat_map(|i| {
                let c = &self.components[i];
                if c.is_applicable(scope) {
                    c.init(scope)
                } else {
                    vec![InitResult::skipped(c.name(), "not applicable")]
                }
            })
            .collect()
    }

    pub fn run_all_deinit(&self, scope: &InitScope) -> Vec<InitResult> {
        // Deinit in reverse priority order
        let mut indices = self.sorted_indices();
        indices.reverse();
        indices
            .into_iter()
            .flat_map(|i| {
                let c = &self.components[i];
                if c.is_applicable(scope) {
                    c.deinit(scope)
                } else {
                    vec![InitResult::skipped(c.name(), "not applicable")]
                }
            })
            .collect()
    }

    pub fn run_all_start(&self) -> Vec<InitResult> {
        self.sorted_indices()
            .into_iter()
            .flat_map(|i| self.components[i].start())
            .collect()
    }

    pub fn run_all_stop(&self) -> Vec<InitResult> {
        let mut indices = self.sorted_indices();
        indices.reverse();
        indices
            .into_iter()
            .flat_map(|i| self.components[i].stop())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Default for InitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        name: &'static str,
        category: &'static str,
        priority: i32,
        applicable_scopes: Option<Vec<InitScope>>,
    }

    impl TestComponent {
        fn new(name: &'static str, priority: i32) -> Self {
            Self { name, category: "test", priority, applicable_scopes: None }
        }

        fn with_scopes(mut self, scopes: Vec<InitScope>) -> Self {
            self.applicable_scopes = Some(scopes);
            self
        }
    }

    impl Initializable for TestComponent {
        fn name(&self) -> &str { self.name }
        fn category(&self) -> &str { self.category }
        fn priority(&self) -> i32 { self.priority }

        fn is_applicable(&self, scope: &InitScope) -> bool {
            match &self.applicable_scopes {
                Some(scopes) => scopes.contains(scope),
                None => true,
            }
        }

        fn init(&self, _scope: &InitScope) -> Vec<InitResult> {
            vec![InitResult::ok(self.name, format!("{} initialized", self.name))]
        }

        fn deinit(&self, _scope: &InitScope) -> Vec<InitResult> {
            vec![InitResult::ok(self.name, format!("{} deinitialized", self.name))]
        }

        fn start(&self) -> Vec<InitResult> {
            vec![InitResult::ok(self.name, format!("{} started", self.name))]
        }

        fn stop(&self) -> Vec<InitResult> {
            vec![InitResult::ok(self.name, format!("{} stopped", self.name))]
        }
    }

    #[test]
    fn test_registry_priority_ordering() {
        let mut reg = InitRegistry::new();
        reg.register(TestComponent::new("C-last", 30));
        reg.register(TestComponent::new("A-first", 10));
        reg.register(TestComponent::new("B-middle", 20));

        let results = reg.run_all_init(&InitScope::Project);
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["A-first", "B-middle", "C-last"]);
    }

    #[test]
    fn test_deinit_reverse_priority() {
        let mut reg = InitRegistry::new();
        reg.register(TestComponent::new("A-first", 10));
        reg.register(TestComponent::new("B-middle", 20));
        reg.register(TestComponent::new("C-last", 30));

        let results = reg.run_all_deinit(&InitScope::Project);
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["C-last", "B-middle", "A-first"]);
    }

    #[test]
    fn test_is_applicable_filtering() {
        let mut reg = InitRegistry::new();
        reg.register(TestComponent::new("project-only", 10).with_scopes(vec![InitScope::Project]));
        reg.register(TestComponent::new("local-only", 20).with_scopes(vec![InitScope::Local]));
        reg.register(TestComponent::new("everywhere", 30));

        let results = reg.run_all_init(&InitScope::Project);
        let statuses: Vec<(&str, InitStatus)> = results
            .iter()
            .map(|r| (r.name.as_str(), r.status))
            .collect();

        assert_eq!(statuses, vec![
            ("project-only", InitStatus::Ok),
            ("local-only", InitStatus::Skipped),
            ("everywhere", InitStatus::Ok),
        ]);
    }

    #[test]
    fn test_start_stop_no_scope() {
        let mut reg = InitRegistry::new();
        reg.register(TestComponent::new("worker", 10));

        let start_results = reg.run_all_start();
        assert_eq!(start_results.len(), 1);
        assert_eq!(start_results[0].message, "worker started");

        let stop_results = reg.run_all_stop();
        assert_eq!(stop_results.len(), 1);
        assert_eq!(stop_results[0].message, "worker stopped");
    }

    #[test]
    fn test_default_empty_impls() {
        struct Minimal;
        impl Initializable for Minimal {
            fn name(&self) -> &str { "minimal" }
            fn category(&self) -> &str { "test" }
        }

        let mut reg = InitRegistry::new();
        reg.register(Minimal);

        assert_eq!(reg.run_all_init(&InitScope::Project).len(), 0);
        assert_eq!(reg.run_all_deinit(&InitScope::Project).len(), 0);
        assert_eq!(reg.run_all_start().len(), 0);
        assert_eq!(reg.run_all_stop().len(), 0);
    }

    #[test]
    fn test_init_result_constructors() {
        let ok = InitResult::ok("n", "m");
        assert_eq!(ok.status, InitStatus::Ok);

        let warn = InitResult::warning("n", "m");
        assert_eq!(warn.status, InitStatus::Warning);

        let err = InitResult::error("n", "m");
        assert_eq!(err.status, InitStatus::Error);

        let skip = InitResult::skipped("n", "m");
        assert_eq!(skip.status, InitStatus::Skipped);
    }

    #[test]
    fn test_registry_len() {
        let mut reg = InitRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.register(TestComponent::new("a", 0));
        reg.register(TestComponent::new("b", 0));
        assert_eq!(reg.len(), 2);
        assert!(!reg.is_empty());
    }
}
