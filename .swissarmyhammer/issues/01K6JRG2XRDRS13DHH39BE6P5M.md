# Fix rule template partial loading - unify with prompt partial loading

## Problem
Rule checking fails with "Partial does not exist" error even though the partial exists:

```
❌ Check failed: Error during rule checking: Failed to render rule template for code-quality/function-length: Template rendering error: liquid: Partial does not exist
  with:
    name=_partials/report-format
from: {% include "_partials/report-format" %}
```

The partial `_partials/report-format` exists but is not being loaded correctly by the rule template renderer.

## Root Cause
Rules and prompts likely have **separate, duplicated template rendering code** that handles partials differently. This violates DRY and leads to inconsistent behavior.

## Investigation Needed

1. **Find prompt partial loading code**
   - How do prompts load partials?
   - Where is the liquid template setup for prompts?
   - How are partial paths resolved?

2. **Find rule partial loading code**
   - How do rules load partials?
   - Where is the liquid template setup for rules?
   - How are partial paths resolved?

3. **Identify the differences**
   - Why do prompts work and rules don't?
   - Are there two separate `liquid::ParserBuilder` setups?
   - Are partial includes registered differently?

## Expected Solution

### Create shared template rendering infrastructure

The solution should:

1. **Extract common template rendering to `swissarmyhammer-common`**
   - Create a shared `TemplateRenderer` or similar
   - Single implementation of partial loading
   - Single implementation of liquid parser configuration
   - Reusable by both rules and prompts

2. **Use the same code for both rules and prompts**
   - Both should use the shared renderer
   - Both should load partials the same way
   - Both should have access to the same partials

3. **Ensure partials are discoverable**
   - Partials should be loaded from all standard locations (builtin, user, project)
   - Partial paths should be resolved consistently
   - Include paths should be registered correctly with liquid parser

## Files to Investigate

- Prompt template rendering: `swissarmyhammer*/src/**/prompt*.rs` or similar
- Rule template rendering: `swissarmyhammer-rules/src/**/*.rs`
- Look for `liquid::ParserBuilder`, `liquid::Parser`, partial includes
- Search for `_partials` directory references

## Success Criteria

- [ ] Rules can use `{% include "_partials/..." %}` successfully
- [ ] Prompts and rules share the same template rendering code
- [ ] No duplication of liquid parser setup
- [ ] Partials loaded from all sources (builtin, user, project)
- [ ] `rule check` command works with rules that use partials
- [ ] Tests verify partial loading works for both rules and prompts

## Priority
High - This blocks using partials in rules, which was the intended design for code reuse.
