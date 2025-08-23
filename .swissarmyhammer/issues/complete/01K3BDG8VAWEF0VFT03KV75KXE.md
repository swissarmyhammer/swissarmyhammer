# Issue

Describe the issue here.


This does not tell me which tool is invalid, so I cannot even begin to fix it.


```
 cargo run
   Compiling swissarmyhammer v0.1.0 (/Users/wballard/github/sah-cli/swissarmyhammer)
   Compiling swissarmyhammer-tools v0.1.0 (/Users/wballard/github/sah-cli/swissarmyhammer-tools)
   Compiling swissarmyhammer-cli v0.1.0 (/Users/wballard/github/sah-cli/swissarmyhammer-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.71s
     Running `target/debug/sah`
⚠  CLI Validation Issues: ⚠  24 of 25 CLI tools are valid (96.0% success rate, 1 validation errors)
Validation warnings (1 issues):
  1. Tool validation warning: Unsupported schema type 'object' for parameter 'environment'. Nested objects are not supported in CLI. Consider flattening the schema or using a string representation.

No command specified. Use --help for usage information.
```
## Problem Analysis

The CLI validation error is caused by the `shell_execute` tool's `environment` parameter being defined as a JSON schema object type:

```json
"environment": {
    "type": "object",
    "description": "Additional environment variables to set (optional)",
    "additionalProperties": {
        "type": "string"
    }
}
```

This is found in `/Users/wballard/github/sah-cli/swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` around line 1207-1213.

The CLI validation system doesn't support nested object types and expects flattened schemas.

## Proposed Solution

Convert the `environment` parameter from an object type to a string representation that can be parsed. The options are:

1. **JSON String Representation**: Change the schema to expect a JSON string that gets parsed into the HashMap
2. **Key=Value Format**: Use a format like "KEY1=value1,KEY2=value2" 
3. **Remove CLI Support**: Mark this parameter as CLI-incompatible

I recommend option 1 (JSON string) as it's the most flexible and maintains full functionality. The change will be:

1. Update the JSON schema to use `"type": "string"` with description indicating JSON format
2. Add parsing logic to convert the JSON string to HashMap when processing the request
3. Update the tool documentation to reflect the string format requirement

## Implementation Steps

1. Modify the schema in `get_tool_definition()` method
2. Add JSON parsing logic in the `execute()` method 
3. Update tests to use the new string format
4. Run `cargo run` to verify the CLI validation passes
## Implementation Complete

Successfully resolved the CLI validation issue. The problem was in the `shell_execute` tool's `environment` parameter being defined as an object type in the JSON schema, which is not supported by the CLI.

### Changes Made

1. **Updated JSON Schema** (`swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:1207-1209`):
   - Changed from `"type": "object"` to `"type": "string"`
   - Updated description to indicate JSON string format expected

2. **Updated Request Structure** (line 35):
   - Changed `environment: Option<std::collections::HashMap<String, String>>` to `environment: Option<String>`

3. **Added JSON Parsing Logic** (lines 1284-1302):
   - Added parsing of JSON string to HashMap before validation and execution
   - Added proper error handling for invalid JSON format
   - Maintained security validation of environment variables

4. **Updated All Tests**:
   - `test_execute_with_all_parameters`
   - `test_execute_with_environment_variables` 
   - `test_environment_variable_security_validation`
   - `test_environment_variable_value_too_long`

### Verification

- `cargo build` - ✅ Compiles successfully
- `cargo run` - ✅ No more CLI validation warnings
- `cargo nextest run shell::execute` - ✅ All tests pass

The CLI now shows no validation issues and all 25 tools pass validation.

## Resolution 

Successfully resolved by converting the environment parameter from JSON schema object type to string type with JSON parsing. The changes made:

1. **Schema Update**: Changed environment parameter from `"type": "object"` to `"type": "string"` in the JSON schema
2. **Runtime Parsing**: Added JSON string to HashMap parsing with proper error handling
3. **Test Updates**: Updated all related tests to use JSON string format
4. **Security Maintained**: All existing environment variable security validations preserved

## Technical Details

- **Location**: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`
- **Schema Change**: Lines 1207-1209 
- **Parsing Logic**: Lines 1284-1302
- **Request Structure**: Updated line 35 to use `Option<String>` instead of `Option<HashMap>`

## Verification

- ✅ `cargo build` - Clean compilation
- ✅ `cargo clippy --all-targets -- -D warnings` - No lint issues
- ✅ `cargo nextest run shell::execute` - All tests pass
- ✅ CLI validation now shows all 25 tools valid (100% success rate)

The solution maintains full functionality while ensuring CLI compatibility by converting the object parameter to a JSON string format that gets parsed at runtime.