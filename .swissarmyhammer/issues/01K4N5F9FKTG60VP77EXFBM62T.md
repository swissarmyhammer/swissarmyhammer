# Complete Domain Separation - Eliminate ALL swissarmyhammer-tools Dependencies on Main Crate

## Goal
Eliminate **ALL 23 imports** from `swissarmyhammer-tools` to the main `swissarmyhammer` crate, achieving complete domain separation.

## Current State: 23 Dependencies Across 7 Areas

### 1. Error System (5+ imports) - IN PROGRESS
**Issue**: `01K4N32CPQYVANC3C6TQQFQPDV`
**Current**: 
```rust
use swissarmyhammer::{Result, SwissArmyHammerError};
use swissarmyhammer::error::SwissArmyHammerError;
```
**Target**: `use swissarmyhammer_common::{Result, SwissArmyHammerError};`

### 2. Shell Security (7+ imports)
**Issue**: `01K4MGNA1YMZX7ND5AS893FQX6`
**Current**: `use swissarmyhammer::shell_security::{ShellSecurityPolicy, ShellSecurityValidator};`
**Target**: `use swissarmyhammer_shell::{ShellSecurityPolicy, ShellSecurityValidator};`

### 3. Test Utilities (4+ imports) 
**Issue**: `01K4MRN7MW4X4W87SWTFEXQG2M`
**Current**: `use swissarmyhammer::test_utils::IsolatedTestHome;`
**Target**: `use swissarmyhammer_common::test_utils::IsolatedTestHome;`

### 4. Prompt System (4+ imports)
**Issue**: `01K4MWFPC5RRZN82F5R6YTAM8K`
**Current**: `use swissarmyhammer::{PromptLibrary, PromptResolver};`
**Target**: `use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};`

### 5. File Watcher (2+ imports)
**Issue**: `01K4MS7BHFH3A6S2A0VZRTQ13M`
**Current**: `use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};`
**Target**: Direct `notify` crate usage

### 6. Outline System (1 import)
**Issue**: `01K4MFS8094J4ZY0XRWAMW3HDX`
**Current**: `use swissarmyhammer::outline::types::OutlineNodeType;`
**Target**: `use swissarmyhammer_outline::OutlineNodeType;`

### 7. Workflow System (1 import)
**Issue**: `01K4N5EH6C55KSY74312ZSC668`
**Current**: `use swissarmyhammer::workflow::{...};`
**Target**: `use swissarmyhammer_workflow::{...};`

## Master Completion Verification

**This MASTER issue is complete when this command returns ZERO results:**

```bash
rg "use swissarmyhammer::" swissarmyhammer-tools/
```

**Success Metrics:**
- **Current**: 23 imports from main crate
- **Target**: 0 imports from main crate
- **Progress**: Issues address all 23 dependencies

## Issue Dependencies (Completion Order)

### Foundation (No Dependencies)
1. `01K4N32CPQYVANC3C6TQQFQPDV` - Error migration to common
2. `01K4MRN7MW4X4W87SWTFEXQG2M` - Test utilities to common
3. `01K4MS7BHFH3A6S2A0VZRTQ13M` - File watcher to direct notify
4. `01K4MWFPC5RRZN82F5R6YTAM8K` - Templating extraction (foundation for prompts)

### Domain Crates (Depends on Foundation)
5. `01K4MGNA1YMZX7ND5AS893FQX6` - Shell domain crate
6. `01K4MFS8094J4ZY0XRWAMW3HDX` - Outline domain crate
7. `01K4N5EH6C55KSY74312ZSC668` - Workflow domain crate (depends on templating)

## Status Tracking

### Completed
- [ ] Error System (5+ deps eliminated)
- [ ] Test Utilities (4+ deps eliminated)  
- [ ] File Watcher (2+ deps eliminated)
- [ ] Shell Security (7+ deps eliminated)
- [ ] Outline System (1+ dep eliminated)
- [ ] Prompt System (4+ deps eliminated)
- [ ] Workflow System (1+ dep eliminated)

### Verification Commands
```bash
# Overall progress check:
echo "Current dependencies: $(rg 'use swissarmyhammer::' swissarmyhammer-tools/ | wc -l)"

# Should be 0 when complete:
rg "use swissarmyhammer::" swissarmyhammer-tools/

# Should find domain crate usage:
rg "use swissarmyhammer_(common|shell|outline|prompts|workflow|templating)" swissarmyhammer-tools/
```

## Benefits When Complete
- ✅ **True Domain Separation**: Each domain is independent  
- ✅ **Reduced Coupling**: Tools don't depend on monolithic main crate
- ✅ **Better Testing**: Domain crates can be tested in isolation
- ✅ **Easier Maintenance**: Clear boundaries and responsibilities
- ✅ **Reusability**: Domain crates can be used by other projects

## Notes
This is the master tracking issue for complete domain separation. All 7 sub-issues must be completed to achieve the goal of zero dependencies from swissarmyhammer-tools to the main crate.