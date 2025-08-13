# Never Fall Back to Main Branch

## Core Principle
**Never automatically fall back to main branch or assume the main branch as a default merge target.**

## Rationale
- Falls back to main defeats the purpose of smart branch detection
- Main branch may not be the correct merge target (could be feature branches, release branches, etc.)
- Users should explicitly handle cases where automatic detection fails
- Better to fail fast with clear error messages than make wrong assumptions

## Implementation
- Use abort files and return errors instead of falling back to main
- Provide detailed error messages explaining why detection failed
- Let users make informed decisions about merge targets

## Examples of What NOT to Do
```rust
// BAD - Don't do this
if no_target_found {
    let main_branch = self.main_branch()?; // NO!
    return Ok(main_branch);
}
```

## Examples of What TO Do
```rust
// GOOD - Do this instead  
if no_target_found {
    create_abort_file(&self.work_dir, "Cannot determine merge target...")?;
    return Err(SwissArmyHammerError::git_operation_failed(...));
}
```

This ensures users are aware of issues and can take appropriate action rather than silently merging to the wrong branch.