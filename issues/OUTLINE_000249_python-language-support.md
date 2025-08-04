# OUTLINE_000249: Python Language Support

Refer to ./specification/outline_tool.md

## Summary

Implement comprehensive Python language support for the outline tool, including extraction of classes, functions, methods, properties, decorators, and docstrings with proper type hint information and module structure handling.

## Context

Python has dynamic typing with optional type hints, distinctive documentation patterns using docstrings, and rich decorator support. The implementation should handle modern Python features including async/await, dataclasses, and type annotations.

## Requirements

### 1. Python Symbol Types
Support all major Python constructs:
- **Classes**: Regular classes with inheritance and metaclasses
- **Functions**: Module-level functions, nested functions, lambdas
- **Methods**: Instance methods, class methods, static methods
- **Properties**: Properties with getters, setters, deleters
- **Variables**: Module-level variables, class variables
- **Decorators**: Function and class decorators
- **Async functions**: Async def functions and async generators
- **Dataclasses**: @dataclass decorated classes with fields
- **Enums**: Enum classes and IntEnum
- **Protocols**: typing.Protocol definitions

### 2. Python-Specific Features
- **Type hints**: Parameter types, return types, variable annotations
- **Default parameters**: Default values and keyword arguments
- **Variadic parameters**: *args and **kwargs
- **Decorators**: Built-in and custom decorators
- **Context managers**: __enter__ and __exit__ methods
- **Magic methods**: __init__, __str__, __repr__, operators
- **Properties**: @property, @classmethod, @staticmethod
- **Async/await**: Async functions, async generators, async context managers

### 3. Docstring Extraction
- **Docstring formats**: Google, NumPy, Sphinx/reStructuredText styles
- **Module docstrings**: Top-level module documentation
- **Class docstrings**: Class-level documentation
- **Method docstrings**: Function and method documentation
- **Type information**: Extract types from docstrings when type hints absent

## Technical Details

### Python Extractor Implementation
```rust
pub struct PythonExtractor {
    query: Query,
}

impl SymbolExtractor for PythonExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode> {
        // Implementation for Python-specific symbol extraction
    }
}
```

### Tree-sitter Queries for Python
Define comprehensive queries:

```scheme
; Class definitions
(class_definition
  name: (identifier) @class.name
  superclasses: (argument_list
    (identifier) @class.bases)*
  body: (block) @class.body)

; Function definitions
(function_definition
  name: (identifier) @function.name
  parameters: (parameters) @function.params
  return_type: (type) @function.return?
  body: (block) @function.body)

; Async function definitions
(async_function_definition
  name: (identifier) @async_function.name
  parameters: (parameters) @async_function.params
  return_type: (type) @async_function.return?
  body: (block) @async_function.body)

; Decorated definitions
(decorated_definition
  (decorator
    (identifier) @decorator.name) @decorator
  definition: [
    (function_definition) @decorated.function
    (class_definition) @decorated.class
    (async_function_definition) @decorated.async_function
  ] @decorated.definition)

; Method definitions
(class_definition
  body: (block
    (function_definition
      name: (identifier) @method.name
      parameters: (parameters
        (identifier) @method.self) @method.params)))

; Property definitions
(decorated_definition
  (decorator
    (identifier) @property.decorator
    (#match? @property.decorator "property|classmethod|staticmethod"))
  definition: (function_definition
    name: (identifier) @property.name))
```

### Signature Generation
Generate accurate Python signatures:

```rust
fn extract_python_signature(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "class_definition" => extract_class_signature(node, source),
        "function_definition" => extract_function_signature(node, source),
        "async_function_definition" => extract_async_function_signature(node, source),
        _ => None,
    }
}
```

Example signatures:
- `class Repository(BaseRepository[T], Generic[T]):`
- `def process_data(items: List[T], *, filter_func: Callable[[T], bool] = None) -> Iterator[T]:`
- `async def fetch_user(user_id: str, session: ClientSession = None) -> Optional[User]:`
- `@property def name(self) -> str:`
- `@classmethod def from_dict(cls, data: Dict[str, Any]) -> 'User':`
- `@dataclass class Config:`

### Docstring Parsing
Extract and parse different docstring formats:

```rust
fn extract_python_docstring(node: &Node, source: &str) -> Option<String> {
    // Parse docstrings in various formats:
    // - Google style
    // - NumPy style  
    // - Sphinx/reStructuredText style
}

#[derive(Debug)]
struct ParsedDocstring {
    description: Option<String>,
    parameters: Vec<Parameter>,
    returns: Option<String>,
    raises: Vec<Exception>,
    examples: Vec<String>,
}
```

## Implementation Steps

1. Create `src/outline/extractors/python.rs`
2. Define Tree-sitter queries for all Python constructs
3. Implement class and function extraction with inheritance
4. Add decorator handling and recognition
5. Implement method classification (instance, class, static)
6. Add type hint extraction and parsing
7. Implement comprehensive docstring parsing
8. Handle async/await and generator functions
9. Add support for dataclasses and protocols
10. Create comprehensive tests with real Python code

## Testing Requirements

### Test Cases
Create test files covering:

**Core Python Features**:
```python
"""User management module.

This module provides classes and functions for managing user data
and authentication within the application.
"""

from typing import Optional, List, Dict, Any, Protocol, Generic, TypeVar
from dataclasses import dataclass, field
from abc import ABC, abstractmethod
import asyncio

T = TypeVar('T')

class UserProtocol(Protocol):
    """Protocol defining user interface."""
    
    id: str
    name: str
    
    def get_permissions(self) -> List[str]:
        """Get user permissions."""
        ...

@dataclass
class User:
    """User data model.
    
    Attributes:
        id: Unique user identifier
        name: User display name
        email: User email address
        permissions: List of user permissions
    """
    id: str
    name: str
    email: str
    permissions: List[str] = field(default_factory=list)
    _internal_data: Dict[str, Any] = field(default_factory=dict, repr=False)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'User':
        """Create user from dictionary data.
        
        Args:
            data: Dictionary containing user data
            
        Returns:
            New User instance
            
        Raises:
            ValueError: If required fields are missing
        """
        if 'id' not in data or 'name' not in data:
            raise ValueError("Missing required fields")
        
        return cls(
            id=data['id'],
            name=data['name'],
            email=data.get('email', ''),
            permissions=data.get('permissions', [])
        )
    
    @property
    def display_name(self) -> str:
        """Get formatted display name."""
        return f"{self.name} ({self.email})"
    
    @display_name.setter
    def display_name(self, value: str) -> None:
        """Set display name (updates name field)."""
        if '(' in value:
            self.name = value.split('(')[0].strip()
    
    def __str__(self) -> str:
        return self.display_name
    
    def __repr__(self) -> str:
        return f"User(id='{self.id}', name='{self.name}')"

class Repository(ABC, Generic[T]):
    """Abstract base repository class.
    
    Generic repository interface for data access operations.
    Supports any data type T that implements the required protocol.
    """
    
    def __init__(self, connection_string: str):
        """Initialize repository with connection.
        
        Args:
            connection_string: Database connection string
        """
        self.connection_string = connection_string
        self._cache: Dict[str, T] = {}
    
    @abstractmethod
    async def find_by_id(self, id: str) -> Optional[T]:
        """Find entity by ID.
        
        Args:
            id: Entity identifier
            
        Returns:
            Entity if found, None otherwise
        """
        pass
    
    @abstractmethod  
    async def save(self, entity: T) -> T:
        """Save entity to repository.
        
        Args:
            entity: Entity to save
            
        Returns:
            Saved entity with updated fields
        """
        pass
    
    async def find_many(self, ids: List[str]) -> List[T]:
        """Find multiple entities by IDs.
        
        Args:
            ids: List of entity identifiers
            
        Returns:
            List of found entities
        """
        results = []
        for id in ids:
            entity = await self.find_by_id(id)
            if entity:
                results.append(entity)
        return results

class UserRepository(Repository[User]):
    """Repository for User entities."""
    
    async def find_by_id(self, id: str) -> Optional[User]:
        """Find user by ID."""
        # Implementation here
        pass
    
    async def save(self, user: User) -> User:  
        """Save user to database."""
        # Implementation here
        pass
    
    async def find_by_email(self, email: str) -> Optional[User]:
        """Find user by email address.
        
        Args:
            email: User email to search for
            
        Returns:
            User if found, None otherwise
        """
        # Implementation here
        pass

@asyncio.coroutine
def legacy_async_function(data: str) -> str:
    """Legacy async function using @asyncio.coroutine."""
    yield from asyncio.sleep(1)
    return data.upper()

async def process_users(
    users: List[User], 
    *,
    filter_func: Optional[Callable[[User], bool]] = None,
    transform_func: Optional[Callable[[User], User]] = None
) -> List[User]:
    """Process list of users with optional filtering and transformation.
    
    Args:
        users: List of users to process
        filter_func: Optional function to filter users
        transform_func: Optional function to transform users
        
    Returns:
        Processed list of users
        
    Example:
        >>> users = [User(id='1', name='John', email='john@example.com')]
        >>> result = await process_users(users, filter_func=lambda u: '@' in u.email)
        >>> len(result)
        1
    """
    result = users[:]
    
    if filter_func:
        result = [user for user in result if filter_func(user)]
    
    if transform_func:
        result = [transform_func(user) for user in result]
    
    return result

def create_user_factory(default_permissions: List[str]) -> Callable[..., User]:
    """Create a factory function for users with default permissions.
    
    Args:
        default_permissions: Default permissions for created users
        
    Returns:
        Factory function that creates users
    """
    def factory(id: str, name: str, email: str = '') -> User:
        return User(
            id=id,
            name=name, 
            email=email,
            permissions=default_permissions[:]
        )
    
    return factory
```

### Expected Output Structure
```yaml
User:
  kind: class
  line: 15
  signature: "@dataclass class User:"
  doc: "User data model."
  children:
    - name: "from_dict"
      kind: classmethod
      signature: "@classmethod def from_dict(cls, data: Dict[str, Any]) -> 'User':"
      doc: "Create user from dictionary data."
      line: 26
    - name: "display_name"
      kind: property
      signature: "@property def display_name(self) -> str:"
      doc: "Get formatted display name."
      line: 42
```

## Integration Points

### Type Hint Processing
- Extract type annotations from function signatures
- Parse complex type expressions (Union, Optional, Generic)
- Handle forward references and string annotations
- Support both typing module and built-in generics (Python 3.9+)

### Decorator Recognition
- Identify built-in decorators (@property, @classmethod, @staticmethod)
- Handle dataclass decorators and field definitions
- Extract custom decorator names and arguments
- Support multiple decorators on single definition

## Performance Considerations

- Optimize queries for common Python patterns
- Efficient handling of large Python modules
- Minimal memory usage during docstring parsing
- Cache frequently used query patterns

## Error Handling

- Graceful handling of syntax errors in Python code
- Recovery from incomplete type annotations
- Clear error messages for malformed docstrings
- Fallback extraction for unknown Python constructs

## Success Criteria

- Accurately extracts all major Python language constructs
- Generates correct signatures with type hints and default values
- Properly extracts and parses docstrings in multiple formats
- Handles decorators, properties, and async functions correctly
- Supports modern Python features (dataclasses, protocols, generics)
- Performance suitable for large Python codebases
- Comprehensive test coverage with real Python project examples

## Dependencies

- `tree-sitter-python` parser
- Existing Tree-sitter infrastructure
- Core outline parser framework
- Standard library components

## Notes

Python's dynamic nature means type information may be incomplete, so the implementation should extract whatever information is available. Pay special attention to docstring parsing as this is often the primary source of documentation in Python code. Consider supporting both Python 2 and 3 syntax patterns where relevant.