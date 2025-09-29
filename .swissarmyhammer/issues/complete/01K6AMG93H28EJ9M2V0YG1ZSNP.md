 `sah prompt list` is missing the source column as in flow list or agent list

## Proposed Solution

After analyzing the codebase, I found that:

1. **Agent list** (standard mode) shows: Name, Description, **Source** with emoji indicators (📦 Built-in, 📁 Project, 👤 User)
2. **Prompt list** (standard mode) shows: Name, Title (missing **Source** column) 
3. **VerbosePromptRow** already has a Source field, but **PromptRow** does not

The issue is in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli/src/commands/prompt/display.rs`:

**Changes needed:**
1. Add `source` field to `PromptRow` struct with appropriate `Tabled` annotation  
2. Update the `prompts_to_display_rows_with_sources` function to use source information for both standard and verbose modes
3. Create a `PromptRow::from_prompt_with_source` method similar to what exists for `VerbosePromptRow`
4. Ensure the emoji-based source display is consistent with agent list ("📦 Built-in", "📁 Project", "👤 User")

This will make the prompt list display consistent with the agent list format.
## Implementation Complete

Successfully added the Source column to `sah prompt list` command with emoji-based indicators matching the format used in `sah agent list` and `sah flow list`.

**Changes made:**
1. ✅ Added `source` field to `PromptRow` struct in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli/src/commands/prompt/display.rs`
2. ✅ Created `PromptRow::from_prompt_with_source` method for emoji-based source display
3. ✅ Updated `prompts_to_display_rows_with_sources` function to use source information for both standard and verbose modes
4. ✅ Updated all test cases to include source field assertions
5. ✅ Added comprehensive test coverage for emoji mapping functionality

**Verified working:**
- `sah prompt list` now shows Name, Title, **Source** columns
- Source column displays emoji indicators: "📦 Built-in", "📁 Project", "👤 User"
- Format matches `sah agent list` and `sah flow list` commands
- All tests pass

The prompt list display is now consistent with the other list commands in the codebase.
## Code Review Improvements Completed

### Changes Made

1. **Extracted emoji constants to module-level** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli/src/commands/prompt/display.rs`:
   - Added `BUILTIN_EMOJI`, `PROJECT_EMOJI`, and `USER_EMOJI` constants
   - Updated `file_source_to_emoji()` function to use constants instead of hardcoded strings
   - Improves maintainability and reduces risk of inconsistencies

2. **Identified emoji mapping duplication** across display modules:
   - `prompt/display.rs` has `file_source_to_emoji()` function
   - `agent/display.rs` has `source_to_emoji()` function  
   - `flow/display.rs` has `file_source_to_emoji()` function
   - All three use identical emoji mapping: "📦 Built-in", "📁 Project", "👤 User"

### Verification Results

✅ **All tests pass**: `cargo nextest run` completed successfully (1014 tests run: 1014 passed)  
✅ **No clippy warnings**: `cargo clippy` completed with no warnings or errors
✅ **Consistent emoji display**: Source column now uses constants for better maintainability

### Technical Decisions

- **Constants over centralization**: Chose to extract constants within the module rather than create a shared utility
- **Reason**: Each display module handles different source types (`AgentSource` vs `FileSource`) making a single shared function complex
- **Future improvement**: Could create a shared trait or macro if more display modules are added

### Code Quality Assessment

The implementation maintains high standards:
- No placeholders, TODOs, or unimplemented code
- Comprehensive test coverage maintained (all existing tests continue to pass)
- Proper error handling and edge case coverage preserved
- Consistent with existing codebase patterns