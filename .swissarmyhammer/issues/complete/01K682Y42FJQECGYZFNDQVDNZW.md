 prompt, flow and agent listings need a source column with an emoji using 📦 Built-in, 📁 Project, 👤 User. these three table displays should be consistent, and leave comments in the code and take a memo to that effect
## Proposed Solution

After analyzing the codebase, I found three listing commands that need consistent source columns with emojis:

1. **Prompt listing** (`swissarmyhammer-cli/src/commands/prompt/display.rs`)
   - Currently: `VerbosePromptRow` has Source column showing file path
   - Needed: Change to show emoji-based source type

2. **Flow listing** (`swissarmyhammer-cli/src/commands/flow/display.rs`)
   - Currently: No source column at all
   - Needed: Add Source column with emoji-based source type

3. **Agent listing** (`swissarmyhammer-cli/src/commands/agent/display.rs`)
   - Currently: Source column shows text ("builtin", "project", "user")
   - Needed: Change to show emoji-based source type

### Implementation Steps:

1. Create a utility function to map source types to emojis:
   - 📦 Built-in = FileSource::Builtin / AgentSource::Builtin  
   - 📁 Project = FileSource::Local / AgentSource::Project
   - 👤 User = FileSource::User / AgentSource::User

2. Update prompt display:
   - Modify `VerbosePromptRow::from()` to use source information from resolver
   - Update prompt list command to pass source mapping to display

3. Update flow display:
   - Add source column to both `WorkflowInfo` and `VerboseWorkflowInfo` 
   - Update flow list command to pass source information to display

4. Update agent display:
   - Modify `AgentRow` and `VerboseAgentRow` to use emoji mapping
   - Update the From implementations

5. Ensure all three displays are consistent in column ordering and styling

6. Add comprehensive code comments explaining the emoji mapping system

7. Create a memo documenting the changes and the mapping system