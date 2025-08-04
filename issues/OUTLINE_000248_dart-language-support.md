# OUTLINE_000248: Dart Language Support

Refer to ./specification/outline_tool.md

## Summary

Implement comprehensive Dart language support for the outline tool, including extraction of classes, mixins, enums, functions, methods, properties, constructors, and Dartdoc comments with proper type information and library/part handling.

## Context

Dart has unique language features including mixins, factory constructors, extension methods, and a distinctive documentation system. The implementation should handle Flutter-specific patterns and modern Dart language features.

## Requirements

### 1. Dart Symbol Types
Support all major Dart constructs:
- **Classes**: Regular classes, abstract classes with inheritance
- **Mixins**: Mixin declarations and usage
- **Enums**: Enum definitions with values and methods
- **Extensions**: Extension methods on existing types
- **Functions**: Top-level functions and local functions
- **Methods**: Instance methods, static methods, operators
- **Constructors**: Named constructors, factory constructors
- **Properties**: Fields, getters, setters
- **Typedefs**: Function type definitions
- **Libraries**: Library declarations and exports
- **Parts**: Part declarations and part of statements

### 2. Dart-Specific Features
- **Access modifiers**: Public (default) and private (_prefixed)
- **Optional parameters**: Named and positional optional parameters
- **Generic types**: Generic classes, methods, and functions
- **Async/await**: Async functions and generators
- **Factory constructors**: Factory keyword and patterns
- **Operator overloading**: Custom operator implementations
- **Extension methods**: Methods added to existing types
- **Late initialization**: late keyword for variables

### 3. Dartdoc Comment Extraction
- **Dartdoc comments**: `///` and `/**` patterns
- **Documentation tags**: @param, @returns, @throws
- **Code examples**: Code blocks in documentation
- **Cross-references**: Links to other code elements

## Technical Details

### Dart Extractor Implementation
```rust
pub struct DartExtractor {
    query: Query,
}

impl SymbolExtractor for DartExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode> {
        // Implementation for Dart-specific symbol extraction
    }
}
```

### Tree-sitter Queries for Dart
Define comprehensive queries:

```scheme
; Class definitions
(class_definition
  name: (identifier) @class.name
  (extends_clause (type_identifier) @class.extends)?
  (with_clause (type_identifier) @class.mixins)*
  (implements_clause (type_identifier) @class.implements)*
  body: (class_body) @class.body)

; Method definitions
(method_signature
  name: (identifier) @method.name
  parameters: (formal_parameter_list) @method.params
  return_type: (type_annotation)? @method.return)

; Constructor definitions
(constructor_signature
  name: (identifier)? @constructor.name
  parameters: (formal_parameter_list) @constructor.params)

; Factory constructors
(factory_constructor_signature
  name: (identifier) @factory.name
  parameters: (formal_parameter_list) @factory.params)

; Mixin definitions
(mixin_declaration
  name: (identifier) @mixin.name
  (on_clause (type_identifier) @mixin.on)*
  body: (class_body) @mixin.body)

; Extension definitions
(extension_declaration
  name: (identifier)? @extension.name
  (on_clause (type_identifier) @extension.on)
  body: (extension_body) @extension.body)
```

### Signature Generation
Generate accurate Dart signatures:

```rust
fn extract_dart_signature(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "class_definition" => extract_class_signature(node, source),
        "mixin_declaration" => extract_mixin_signature(node, source),
        "extension_declaration" => extract_extension_signature(node, source),
        "method_signature" => extract_method_signature(node, source),
        "constructor_signature" => extract_constructor_signature(node, source),
        "factory_constructor_signature" => extract_factory_signature(node, source),
        _ => None,
    }
}
```

Example signatures:
- `class Repository<T> extends BaseRepository<T> with CacheMixin implements DataSource<T>`
- `mixin ValidationMixin<T> on BaseModel<T>`
- `extension StringExtensions on String`
- `factory User.fromJson(Map<String, dynamic> json)`
- `Future<List<T>> fetchData<T>({required String endpoint, Map<String, String>? headers})`
- `void operator []=(int index, T value)`

### Dartdoc Integration
Extract comprehensive documentation:
```rust
fn extract_dartdoc_comment(node: &Node, source: &str) -> Option<String> {
    // Parse Dartdoc comments and extract structured information
    // Handle /// comments and /** */ blocks
}
```

## Implementation Steps

1. Create `src/outline/extractors/dart.rs`
2. Define Tree-sitter queries for all Dart constructs
3. Implement class, mixin, and extension extraction
4. Add constructor and factory constructor handling
5. Implement method and property extraction with proper signatures
6. Add async/await and generator function support
7. Implement Dartdoc comment parsing
8. Handle library and part declarations
9. Create comprehensive tests with real Dart/Flutter code

## Testing Requirements

### Test Cases
Create test files covering:

**Core Dart Features**:
```dart
/// Repository for managing user data
abstract class UserRepository<T extends User> 
    extends BaseRepository<T> 
    with CacheMixin<T> 
    implements DataSource<T> {
  
  /// Create a new user repository
  /// 
  /// [cacheSize] specifies the maximum cache size
  UserRepository({int cacheSize = 100});
  
  /// Factory constructor for creating from configuration
  factory UserRepository.fromConfig(Config config) {
    return DatabaseUserRepository<T>(config);
  }
  
  /// Find user by ID
  /// 
  /// Returns [null] if user not found
  /// Throws [UserNotFoundException] if ID is invalid
  Future<T?> findById(String id);
  
  /// Save user data
  /// 
  /// [user] the user to save
  /// [options] optional save configuration
  Future<T> save(T user, {SaveOptions? options});
  
  /// Operator overload for array-like access
  T? operator [](String id) => findByIdSync(id);
}

/// Mixin for caching functionality
mixin CacheMixin<T> on BaseRepository<T> {
  final Map<String, T> _cache = {};
  
  /// Get item from cache
  T? getCached(String key) => _cache[key];
  
  /// Store item in cache
  void setCached(String key, T value) {
    _cache[key] = value;
  }
}

/// Extension methods for String validation
extension StringValidation on String {
  /// Check if string is a valid email
  bool get isValidEmail {
    return RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(this);
  }
  
  /// Capitalize first letter
  String get capitalized {
    if (isEmpty) return this;
    return '${this[0].toUpperCase()}${substring(1)}';
  }
}

/// Enum for user roles
enum UserRole {
  admin('Administrator'),
  user('Regular User'),
  guest('Guest User');
  
  const UserRole(this.displayName);
  
  /// Human-readable display name
  final String displayName;
  
  /// Check if role has admin privileges
  bool get hasAdminPrivileges => this == UserRole.admin;
}

/// Process user data asynchronously
/// 
/// [users] list of users to process
/// [processor] function to apply to each user
/// Returns stream of processed results
Stream<ProcessResult> processUsers(
  List<User> users,
  Future<ProcessResult> Function(User) processor,
) async* {
  for (final user in users) {
    yield await processor(user);
  }
}
```

### Expected Output Structure
```yaml
UserRepository:
  kind: class
  line: 2
  signature: "abstract class UserRepository<T extends User> extends BaseRepository<T> with CacheMixin<T> implements DataSource<T>"
  doc: "Repository for managing user data"
  children:
    - name: "UserRepository"
      kind: constructor
      signature: "UserRepository({int cacheSize = 100})"
      doc: "Create a new user repository"
      line: 8
    - name: "fromConfig"
      kind: factory
      signature: "factory UserRepository.fromConfig(Config config)"
      doc: "Factory constructor for creating from configuration"
      line: 12
    - name: "findById"
      kind: method
      signature: "Future<T?> findById(String id)"
      doc: "Find user by ID"
      line: 18
```

## Integration Points

### Library and Part Handling
- Extract library declarations and documentation
- Handle part of and part statements
- Track imports and exports
- Support Flutter-specific import patterns

### Flutter-Specific Patterns
- Widget class hierarchies
- State management patterns
- Build method extraction
- Widget property extraction

## Performance Considerations

- Optimize queries for common Dart/Flutter patterns
- Efficient handling of large Flutter application files
- Minimal memory usage during generic type parsing
- Cache frequently used query patterns

## Error Handling

- Graceful handling of incomplete Dart syntax
- Recovery from malformed generic specifications
- Clear error messages for invalid constructors
- Fallback extraction for unknown Dart features

## Success Criteria

- Accurately extracts all major Dart language constructs
- Generates correct signatures with generics and optional parameters
- Properly extracts and formats Dartdoc comments
- Handles mixins, extensions, and factory constructors correctly
- Supports Flutter-specific patterns and widgets
- Performance suitable for large Flutter applications
- Comprehensive test coverage with real Dart/Flutter examples

## Dependencies

- `tree-sitter-dart` parser
- Existing Tree-sitter infrastructure
- Core outline parser framework
- Standard library components

## Notes

Dart has unique features like mixins and factory constructors that need special handling. The implementation should also consider Flutter-specific patterns since many Dart projects are Flutter applications. Pay attention to the distinction between sync and async methods, and handle generator functions appropriately.