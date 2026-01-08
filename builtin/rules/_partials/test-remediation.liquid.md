{% partial %}

## Alternative Approaches

If a test genuinely has issues:

1. **Factor into smaller tests** - Break one large test into multiple focused tests
2. **Optimize the test** - Make it faster through better setup/teardown
3. **Fix the underlying issue** - If it's too slow, maybe the code is the problem
4. **Use proper test infrastructure** - Fixtures, helpers, parallel execution

## Bottom Line

Every test should be run when we run tests.
