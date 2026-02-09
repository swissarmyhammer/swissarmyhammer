---
name: no-type-complexity
description: Flatten deeply nested generic types into named type aliases
---

# No Type Complexity

When a type signature requires horizontal scrolling or squinting to parse,
it needs a name. dtolnay's code introduces type aliases as soon as a generic
type nests more than two levels deep.

## What to Check

Look for complex type expressions in signatures, struct fields, and let bindings:

1. **Deeply nested generics**: Types with three or more levels of nesting,
   like `Arc<Mutex<HashMap<String, Vec<Item>>>>`. Should be broken into
   named type aliases.

2. **Repeated complex types**: The same multi-generic type appearing in
   multiple function signatures. Should be a type alias used everywhere.

3. **Long function signatures**: Return types or parameter types that make
   the function signature hard to read at a glance.

4. **Trait objects with multiple bounds**: `Box<dyn Fn(Request) -> Response + Send + Sync + 'static>`
   should have a type alias.

5. **Closure types in signatures**: Complex closure types that obscure the
   function's purpose.

## What Passes

- `type Db = Arc<Mutex<HashMap<String, Record>>>`
- `type Handler = Box<dyn Fn(Request) -> Response + Send + Sync>`
- `type Result<T> = std::result::Result<T, MyError>` (the classic dtolnay pattern from anyhow)
- Simple two-level generics: `Vec<String>`, `HashMap<String, Value>`, `Option<Vec<u8>>`
- Generic types that are well-known and readable: `Arc<Mutex<T>>` for a single `T`

## What Fails

- `fn process(data: Arc<RwLock<HashMap<String, Vec<(usize, String)>>>>)`
  without a type alias
- `HashMap<String, Vec<Box<dyn Fn() -> Result<(), Box<dyn Error>>>>>` inline
- The same `Arc<Mutex<HashMap<K, V>>>` type written out in 4 different functions
- `fn handler() -> Pin<Box<dyn Future<Output = Result<Response, Box<dyn Error + Send + Sync>>> + Send>>`
- Struct fields with types that take more than ~60 characters

## Why This Matters

syn uses type aliases extensively to keep function signatures readable.
`type Result<T> = std::result::Result<T, Error>` and
`type ParseStream<'a> = &'a ParseBuffer<'a>` mean that
`fn parse(input: ParseStream) -> Result<Self>` reads like prose while
hiding substantial generic complexity behind meaningful names.
