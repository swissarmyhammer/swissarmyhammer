# CRITICAL: Do Not Skip Tests Because They Are "Slow"

## Policy

**NEVER mark tests with `#[ignore]` just because they take time to run.**

The excuse "this test is slow" is NOT acceptable. Tests must run by default.

## What This Means

- ❌ NO `#[ignore = "slow test"]`
- ❌ NO `#[ignore = "downloads LLM model"]`  
- ❌ NO `#[ignore = "spawns subprocess"]`
- ❌ NO performance-based test skipping of ANY kind

## Alternative Approaches

If a test genuinely has issues:

1. **Factor into smaller tests** - Break one large test into multiple focused tests
2. **Optimize the test** - Make it faster through better setup/teardown
3. **Fix the underlying issue** - If it's too slow, maybe the code is the problem
4. **Use proper test infrastructure** - Fixtures, helpers, parallel execution

## Bottom Line

Every test should run in CI by default. Performance is NOT a reason to skip validation.

If you're adding `#[ignore]` for performance reasons, you're doing it wrong.