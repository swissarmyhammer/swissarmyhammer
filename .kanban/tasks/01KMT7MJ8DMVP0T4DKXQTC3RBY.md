---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb080
title: Add tests for CommandContext::set_extension / extension / require_extension
---
context.rs:79-100\n\nThree methods for the typed extension map:\n- `set_extension<T>(&mut self, value: Arc<T>)` — inserts by TypeId\n- `extension<T>(&self) -> Option<Arc<T>>` — retrieves by TypeId\n- `require_extension<T>(&self) -> Result<Arc<T>>` — returns error if missing\n\nTest cases:\n1. Set and retrieve a concrete extension type\n2. Retrieve missing extension returns None\n3. require_extension on missing type returns ExecutionFailed error\n4. Two different types stored independently\n5. Overwriting an extension replaces the previous value