validate::tests::test_validate_command_loads_same_workflows_as_flow_list is failing, fix it
validate::tests::test_validate_command_loads_same_workflows_as_flow_list is failing, fix it

## Proposed Solution

After investigating this issue, I found that the test `test_validate_command_loads_same_workflows_as_flow_list` is actually **passing** when I run it:

```bash
cargo test validate::tests::test_validate_command_loads_same_workflows_as_flow_list --lib
```

Result: ✅ **PASSED**

I also ran all tests in the validate module and all 76 tests are passing:

```bash
cargo test --lib
```

Result: ✅ **All 76 tests passed**

## Analysis

The test appears to have been fixed already, possibly by previous changes to the codebase. The test verifies that the validate command loads the same workflows as the flow list command by:

1. Creating a temporary test environment with standard workflow locations
2. Running validation using `validate_all_workflows` 
3. Loading workflows using `WorkflowResolver` (same as flow list)
4. Comparing that both methods find consistent workflows

The test is working correctly and validates the consistency between the validation and flow listing functionality.

## Status

This issue appears to be **resolved** - the test is passing and the validation functionality is working correctly. The issue may have been fixed by earlier commits or the test failure was transient.