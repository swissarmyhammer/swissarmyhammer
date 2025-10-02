Never ignore a test.

## Alternative Approaches

If a test genuinely has issues:

1. **Factor into smaller tests** - Break one large test into multiple focused tests
2. **Optimize the test** - Make it faster through better setup/teardown
3. **Fix the underlying issue** - If it's too slow, maybe the code is the problem
4. **Use proper test infrastructure** - Fixtures, helpers, parallel execution

## Bottom Line

Every test should run in CI by default. Performance is NOT a reason to skip validation.

If you're adding `#[ignore]` for performance reasons, you're doing it wrong.
