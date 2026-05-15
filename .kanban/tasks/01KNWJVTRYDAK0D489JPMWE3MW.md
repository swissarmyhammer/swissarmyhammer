---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffdc80
title: Add test for shelltool-cli banner::print_banner
---
shelltool-cli/src/banner.rs:91-95

Coverage: 74/80 (92.5%) — uncovered lines 91-95, 107

```rust
pub fn print_banner() {
    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = io::stdout().lock();
    render_banner(&mut out, use_color);
}
```

The public `print_banner` wrapper is uncovered (existing tests call `render_banner` directly with a `Vec<u8>` buffer).

Also uncovered: line 107, the `1 =>` arm of `should_show_banner` that returns `io::stdin().is_terminal()` for the no-user-args case.

**What to test:**
- A smoke test that calls `print_banner()` and asserts it does not panic. It writes to a real stdout lock, so in a test harness it should be harmless. Set `NO_COLOR=1` in the test to force the uncolored branch so the test is deterministic.
- For `should_show_banner` line 107: pass a single-element `vec!["shelltool".to_string()]` and assert the result matches `io::stdin().is_terminal()` (or just assert the function runs without panic, since terminal state in tests is environment-dependent).

Both tests go in the existing `#[cfg(test)] mod tests` block at the bottom of banner.rs. #coverage-gap