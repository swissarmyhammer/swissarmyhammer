//! Leadership-contention policy seam.
//!
//! A documented hook for excluding processes that must never lead. It is a
//! policy seam, not a heuristic: it defaults to permitting leadership and only
//! reads an explicit opt-out env var, so it never *guesses* at a process's role.

/// Whether THIS process is permitted to contend for leadership.
///
/// Subagent-spawned `sah serve` processes must never lead (only a root/top-level
/// server should). TODO(^d8vae11): there is currently NO reliable root-vs-subagent
/// signal in this codebase — the MCP config writes an empty env for `sah serve`,
/// and the stdio children that must be excluded are spawned by Claude Code's own
/// Task tool (outside this repo), reading one shared `.mcp.json`, so no per-agent
/// env var is separable; PPID ancestry is fragile. Until a reliable signal exists
/// (e.g. the harness setting `SAH_SUBAGENT=1` on subagent MCP configs), this
/// defaults to permitting leadership. When such a signal lands, gate it here.
pub fn may_contend_for_leadership() -> bool {
    // Honor an explicit opt-out if the harness ever sets it; default: allowed.
    std::env::var("SAH_SUBAGENT")
        .map(|v| v != "1")
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With `SAH_SUBAGENT=1` set, contention is forbidden; with it unset (the
    /// default) or any other value, it is allowed. The env var is restored to
    /// its prior state at the end so this test does not leak global state.
    ///
    /// Reads are taken once per state to avoid racing other tests that may run
    /// in parallel; the var is set+observed+removed within this single test.
    #[test]
    fn test_may_contend_honors_explicit_opt_out() {
        let prior = std::env::var("SAH_SUBAGENT").ok();

        std::env::set_var("SAH_SUBAGENT", "1");
        let forbidden = may_contend_for_leadership();

        std::env::set_var("SAH_SUBAGENT", "0");
        let allowed_other = may_contend_for_leadership();

        std::env::remove_var("SAH_SUBAGENT");
        let allowed_unset = may_contend_for_leadership();

        // Restore prior state before asserting.
        match prior {
            Some(v) => std::env::set_var("SAH_SUBAGENT", v),
            None => std::env::remove_var("SAH_SUBAGENT"),
        }

        assert!(!forbidden, "SAH_SUBAGENT=1 must forbid contention");
        assert!(allowed_other, "a non-\"1\" value must permit contention");
        assert!(
            allowed_unset,
            "unset must permit contention (default allow)"
        );
    }
}
