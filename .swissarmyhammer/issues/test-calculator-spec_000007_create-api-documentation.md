# Step 7: Create API Documentation

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Create comprehensive API documentation that meets specification requirement R3.

## Requirements (from spec)
- All endpoints must be documented
- Include example requests
- Include example responses

## Documentation to Create

### 1. Module Documentation
Add doc comments to all public types and functions in the calculator module.

### 2. API Documentation File
Create `swissarmyhammer-calculator/README.md` with:

```markdown
# Calculator API Documentation

## Overview
Simple HTTP calculator API providing basic arithmetic operations.

## Endpoints

### POST /calculator/add

Adds two numbers together.

**Query Parameters:**
- `a` (required): First number
- `b` (required): Second number

**Success Response (HTTP 200):**
```json
{
  "result": 8.0
}
```

**Error Response (HTTP 400):**
```json
{
  "error": "Invalid input: 'abc' is not a valid number",
  "status": 400
}
```

**Example Requests:**
```bash
# Valid request
curl "http://localhost:8080/calculator/add?a=5&b=3"

# Invalid request
curl "http://localhost:8080/calculator/add?a=abc&b=5"
```

## Error Handling
The API returns HTTP 400 for:
- Non-numeric inputs
- Missing parameters
- Invalid number formats (NaN, Infinity)
```

### 3. Integration into Main Documentation
Update main project README if appropriate to mention calculator API capability.

## Acceptance Criteria
- [ ] All public APIs have doc comments
- [ ] README.md created with complete examples
- [ ] Example requests are copy-pasteable
- [ ] Example responses show actual JSON format
- [ ] Error cases documented
- [ ] Documentation matches actual implementation
- [ ] Code examples tested and verified

## Dependencies
Requires: Step 6 (integration tests passing)

## Next Step
Final verification of all requirements
