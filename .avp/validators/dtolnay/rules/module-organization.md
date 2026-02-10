---
name: module-organization
description: Organize modules by domain with one primary type per file and minimal lib.rs
---

# Module Organization

dtolnay's crates follow a clear structural pattern: each file has a single
responsibility, `lib.rs` is a table of contents (not an implementation file),
and the module tree reflects domain concepts. You should be able to find any
type by its name alone because the file is named after it.

## What to Check

Look at module structure, file size, and `lib.rs` content:

1. **Oversized files**: A file with multiple public structs, enums, or traits
   that serve different purposes. Each primary type should live in its own file
   named after it (e.g., `struct Parser` lives in `parser.rs`).

2. **Fat `lib.rs`**: A `lib.rs` that contains implementation code beyond
   re-exports and top-level documentation. `lib.rs` should be `pub mod`
   declarations, `pub use` re-exports, and `//!` doc comments.

3. **`mod.rs` with logic**: A `mod.rs` that does more than declare submodules
   and re-export their public items. Keep `mod.rs` as a routing file.

4. **Technical-layer organization**: Modules named `utils`, `helpers`, `types`,
   `models`, or `common` that group unrelated items by technical role. Organize
   by what the code *does* in the domain instead.

5. **Deeply nested modules**: More than 3 levels of nesting usually means the
   abstraction boundaries need rethinking. `crate::foo::bar::baz::qux` is a
   code smell.

## What Passes

- `lib.rs` with only `pub mod`, `pub use`, and `//!` documentation
- `parser.rs` containing `pub struct Parser` and its `impl` blocks
- `error.rs` containing the crate's error types (multiple related error types
  in one file is fine -- they're one concept)
- `mod.rs` that re-exports: `mod parser; pub use parser::Parser;`
- Domain-oriented modules: `tokenizer`, `ast`, `visitor`, `codegen`
- Small helper functions private to the file that contains the type they support

## What Fails

- `lib.rs` with 500 lines of `impl` blocks
- A single `types.rs` file containing 10 unrelated structs
- `mod.rs` with `impl` blocks for types defined in child modules
- `utils.rs` as a dumping ground for unrelated functions
- `helpers/mod.rs` re-exporting 20 items from 15 submodules
- A file named `handler.rs` containing `struct Handler`, `struct Request`,
  `struct Response`, `struct Middleware`, and `struct Router`
- Module paths like `crate::core::internal::processing::pipeline::stage`

## Why This Matters

syn has over 200 AST node types. Each one lives in a file you can find by
name. `Expr` is in `expr.rs`. `Item` is in `item.rs`. `Type` is in `ty.rs`.
When dtolnay needs to modify how `ExprMatch` works, he opens `expr.rs` --
not `types.rs` or `ast.rs` or `nodes.rs`. The module tree *is* the
architecture diagram.
