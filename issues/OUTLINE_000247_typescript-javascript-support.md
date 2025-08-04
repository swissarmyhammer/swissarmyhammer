# OUTLINE_000247: TypeScript/JavaScript Language Support

Refer to ./specification/outline_tool.md

## Summary

Implement comprehensive TypeScript and JavaScript language support for the outline tool, including extraction of classes, interfaces, functions, methods, properties, types, and JSDoc comments with proper type information and export/import handling.

## Context

TypeScript and JavaScript share syntax patterns but TypeScript adds static typing. The implementation should handle both languages while extracting maximum type information from TypeScript files and providing appropriate fallbacks for JavaScript.

## Requirements

### 1. TypeScript/JavaScript Symbol Types
Support all major language constructs:
- **Classes**: ES6 classes with inheritance, methods, properties
- **Interfaces**: TypeScript interface definitions
- **Types**: Type aliases, union types, intersection types
- **Functions**: Regular functions, arrow functions, async functions
- **Methods**: Class methods, getters, setters
- **Properties**: Class properties, object properties
- **Variables**: let, const, var declarations
- **Enums**: TypeScript enums
- **Namespaces**: TypeScript namespaces and modules
- **Decorators**: Class and method decorators

### 2. TypeScript-Specific Features
- **Type annotations**: Parameter types, return types, property types
- **Generics**: Generic functions, classes, and interfaces
- **Access modifiers**: public, private, protected, readonly
- **Optional properties**: Properties with ? modifier
- **Index signatures**: [key: string]: value patterns
- **Conditional types**: Complex type expressions
- **Mapped types**: Advanced type transformations

### 3. JavaScript-Specific Handling
- **Function declarations**: Both function and arrow syntax
- **Object patterns**: Object literals with methods
- **Prototype methods**: Traditional prototype-based inheritance
- **Module patterns**: CommonJS, ES modules
- **Closure patterns**: IIFE and closure-based encapsulation

### 4. JSDoc Comment Extraction
- **JSDoc comments**: `/** */` block comments
- **Type information**: @param, @returns, @type annotations
- **Documentation tags**: @description, @example, @since
- **TypeScript integration**: JSDoc as type source for JS files

## Technical Details

### TypeScript/JavaScript Extractor Implementation
```rust
pub struct TypeScriptExtractor {
    query: Query,
    is_typescript: bool,
}

impl SymbolExtractor for TypeScriptExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode> {
        // Handle both TypeScript and JavaScript
    }
}
```

### Tree-sitter Queries
Define queries for common patterns:

```scheme
; Class definitions
(class_declaration
  name: (type_identifier) @class.name
  superclass: (extends_clause (identifier) @class.extends)?
  body: (class_body
    (method_definition
      name: (property_identifier) @method.name
      parameters: (formal_parameters) @method.params)
    (field_definition
      property: (property_identifier) @field.name)))

; Interface definitions (TypeScript)
(interface_declaration
  name: (type_identifier) @interface.name
  body: (object_type
    (property_signature
      name: (property_identifier) @property.name
      type: (type_annotation) @property.type)))

; Function declarations
(function_declaration
  name: (identifier) @function.name
  parameters: (formal_parameters) @function.params
  return_type: (type_annotation)? @function.return)

; Arrow functions
(arrow_function
  parameters: (formal_parameters) @arrow.params
  return_type: (type_annotation)? @arrow.return
  body: (_) @arrow.body)
```

### Signature Generation
Generate accurate signatures for different constructs:

```rust
fn extract_typescript_signature(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "class_declaration" => extract_class_signature(node, source),
        "interface_declaration" => extract_interface_signature(node, source),
        "function_declaration" => extract_function_signature(node, source),
        "method_definition" => extract_method_signature(node, source),
        "type_alias_declaration" => extract_type_alias_signature(node, source),
        _ => None,
    }
}
```

Example signatures:
- `class Repository<T> implements DataSource<T>`
- `interface User { id: number; name: string; email?: string }`
- `function processData<T>(items: T[]): Promise<ProcessResult<T>>`
- `type Handler<T> = (data: T) => Promise<void>`
- `async getUserById(id: string): Promise<User | null>`

### JSDoc Integration
Extract comprehensive documentation:
```rust
fn extract_jsdoc_comment(node: &Node, source: &str) -> Option<String> {
    // Parse JSDoc comments and extract structured information
    // Handle @param, @returns, @example, etc.
}
```

## Implementation Steps

1. Create `src/outline/extractors/typescript.rs`
2. Define Tree-sitter queries for TypeScript and JavaScript
3. Implement class and interface extraction
4. Add function and method signature generation
5. Implement TypeScript type annotation handling
6. Add JSDoc comment parsing and extraction
7. Handle module and namespace declarations
8. Create comprehensive tests with real TypeScript/JavaScript code

## Testing Requirements

### Test Cases
Create test files covering:

**TypeScript Features**:
```typescript
/**
 * User repository interface
 */
interface UserRepository<T extends User> {
    /** Find user by ID */
    findById(id: string): Promise<T | null>;
    
    /** Save user data */
    save(user: Partial<T>): Promise<T>;
}

/**
 * Implementation of user repository
 */
class DatabaseUserRepository implements UserRepository<DatabaseUser> {
    private readonly connection: Connection;
    
    constructor(connection: Connection) {
        this.connection = connection;
    }
    
    async findById(id: string): Promise<DatabaseUser | null> {
        // Implementation
    }
    
    async save(user: Partial<DatabaseUser>): Promise<DatabaseUser> {
        // Implementation
    }
}

/**
 * Process user data
 * @param users Array of users to process
 * @returns Promise resolving to processed results
 */
export async function processUsers(
    users: User[]
): Promise<ProcessResult[]> {
    // Implementation
}

/**
 * Event handler type
 */
export type EventHandler<T> = (event: T) => void | Promise<void>;
```

**JavaScript Features**:
```javascript
/**
 * User service class
 * @class
 */
class UserService {
    /**
     * Create a new user service
     * @param {Object} config - Configuration object
     */
    constructor(config) {
        this.config = config;
    }
    
    /**
     * Get user by ID
     * @param {string} id - User ID
     * @returns {Promise<Object|null>} User object or null
     */
    async getUserById(id) {
        // Implementation
    }
}

/**
 * Utility function for processing data
 * @param {Array} items - Items to process
 * @param {Function} processor - Processing function
 * @returns {Array} Processed items
 */
function processItems(items, processor) {
    return items.map(processor);
}
```

### Expected Output Structure
```yaml
UserRepository:
  kind: interface
  line: 4
  signature: "interface UserRepository<T extends User>"
  doc: "User repository interface"
  children:
    - name: "findById"
      kind: method
      signature: "findById(id: string): Promise<T | null>"
      doc: "Find user by ID"
      line: 7
```

## Integration Points

### Language Detection
- Detect TypeScript vs JavaScript from file extension
- Configure parser appropriately for each language
- Handle .tsx files (React with TypeScript)
- Support .jsx files (React with JavaScript)

### Type Information Extraction
- Extract TypeScript type annotations accurately
- Parse generic type parameters and constraints
- Handle union and intersection types
- Extract JSDoc type information for JavaScript

## Performance Considerations

- Optimize queries for common TypeScript/JavaScript patterns
- Efficient handling of large application files
- Minimal memory usage during type annotation parsing
- Cache frequently used query patterns

## Error Handling

- Graceful handling of invalid TypeScript syntax
- Recovery from incomplete type annotations
- Clear error messages for malformed code
- Fallback extraction for unknown constructs

## Success Criteria

- Accurately extracts all major TypeScript and JavaScript constructs
- Generates correct signatures with full type information
- Properly extracts and formats JSDoc comments
- Handles generics, unions, and complex types correctly
- Distinguishes between TypeScript and JavaScript appropriately
- Performance suitable for large application codebases
- Comprehensive test coverage with real project examples

## Dependencies

- `tree-sitter-typescript` parser
- `tree-sitter-javascript` parser
- Existing Tree-sitter infrastructure
- Core outline parser framework
- JSDoc parsing utilities

## Notes

TypeScript has complex type system features that should be represented accurately in the outline. Consider handling advanced features like conditional types, mapped types, and template literal types. JavaScript support should focus on extracting maximum information from JSDoc comments and structural patterns.