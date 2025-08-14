# Workflow Parameters Specification

## Overview

This specification outlines how workflow parameters should be implemented to provide consistency with prompt parameters. The goal is to make workflow parameters work and feel identical to prompt parameters, ensuring a unified user experience across the SwissArmyHammer system.

## Current State

Currently, workflows support parameters through:
- Ad-hoc `## Parameters` sections in markdown content
- Liquid template variables in action strings (`{{ name }}`)
- CLI `--set` arguments for parameter passing
- No formal parameter validation or type checking

## Proposed Design

### 1. Frontmatter Parameter Definition

Workflow parameters should be defined in YAML frontmatter using the same structure as prompts:

```yaml
---
title: Greeting Workflow
description: A workflow that greets someone
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
  - name: language
    description: The language to use for greeting
    required: false
    type: string
    default: English
    choices:
      - English
      - Spanish
      - French
  - name: enthusiastic
    description: Whether to use enthusiastic greeting
    required: false
    type: boolean
    default: false
---
```

### 2. Parameter Type Support

Support the same parameter types as prompts:
- `string` - Text input
- `boolean` - True/false values
- `number` - Numeric values
- `choice` - Selection from predefined options
- `multi_choice` - Multiple selections from options

### 3. CLI Parameter Handling

#### Option 1: Named Switches (Preferred)
```bash
# Required parameters as positional or named args
sah flow run greeting --person-name "John" --language "Spanish" --enthusiastic

# Short form support
sah flow run greeting -n "John" -l "Spanish" -e
```

#### Option 2: Interactive Prompting
When parameters are missing, prompt the user interactively:
```bash
sah flow run greeting
? Enter person_name (The name of the person to greet): John
? Select language (default: English): 
  > English
    Spanish  
    French
? Enable enthusiastic greeting? (y/N): y
```

#### Option 3: Mixed Mode
Support both approaches - use provided switches, prompt for missing required parameters.

### 4. Backward Compatibility

#### Migration Strategy for Existing Workflows

1. **builtin/workflows/greeting.md**
   - Move parameter definitions from markdown to frontmatter
   - Keep existing liquid template usage in actions
   - Maintain CLI `--set` support during transition

2. **builtin/workflows/plan.md** 
   - Add `plan_filename` parameter to frontmatter
   - Update documentation to show new parameter format

#### Legacy Support
- None, just convert the builtin workflows during implementation

### 5. Implementation Components

#### 5.1 Workflow Parser Updates
- Extend `parse_front_matter()` to handle `parameters` field
- Add parameter validation during workflow loading
- Support parameter inheritance for sub-workflows

#### 5.2 CLI Integration
- Add parameter parsing to workflow commands
- Implement interactive prompting system
- Generate help text from parameter definitions
- Make sure to create ONE set of code for parameter handling between prompt and workflow and share it

#### 5.3 Parameter Resolution
- Resolve parameters before liquid template rendering
- Support parameter defaults and validation
- Provide clear error messages for missing/invalid parameters

#### 5.4 Template Integration
- Parameters available as liquid template variables
- Same syntax as current: `{{ parameter_name }}`
- Support liquid filters and defaults: `{{ language | default: 'English' }}`

### 6. Enhanced Features

#### 6.1 Parameter Validation
```yaml
parameters:
  - name: email
    type: string
    pattern: '^[^@]+@[^@]+\.[^@]+$'
    description: Valid email address
  - name: port
    type: number
    min: 1
    max: 65535
    description: Network port number
```

#### 6.2 Conditional Parameters
```yaml
parameters:
  - name: deploy_env
    type: choice
    choices: [dev, staging, prod]
  - name: prod_confirmation
    type: boolean
    required: true
    condition: "deploy_env == 'prod'"
    description: Confirm production deployment
```

#### 6.3 Parameter Groups
```yaml
parameter_groups:
  - name: deployment
    description: Deployment configuration
    parameters: [deploy_env, region, instance_count]
  - name: security
    description: Security settings  
    parameters: [enable_ssl, cert_path]
```

### 7. User Experience Goals

1. **Consistency**: Identical parameter handling between prompts and workflows
2. **Discoverability**: `sah flow run <workflow> --help` shows all parameters
3. **Validation**: Clear error messages for invalid parameters
4. **Flexibility**: Support both CLI switches and interactive prompting
5. **Documentation**: Auto-generated parameter documentation

### 8. Implementation Phases

#### Phase 1: Core Infrastructure
- Update workflow parser for frontmatter parameters
- Add CLI parameter parsing for workflows
- Basic parameter validation and resolution

#### Phase 2: Enhanced CLI Experience  
- Interactive parameter prompting
- Auto-generated help text
- Parameter completion support

#### Phase 3: Advanced Features
- Parameter validation rules (patterns, ranges)
- Conditional parameters
- Parameter groups and organization

#### Phase 4: Migration and Cleanup
- Migrate existing workflows to new format
- Remove legacy `--set` support
- Update documentation and examples

### 9. Testing Strategy

#### 9.1 Unit Tests
- Parameter parsing from frontmatter
- Parameter validation logic
- Template variable resolution

#### 9.2 Integration Tests
- CLI parameter handling
- Interactive prompting flows
- Workflow execution with parameters

#### 9.3 Migration Tests
- Backward compatibility during transition
- Legacy workflow support
- Parameter format conversion

### 10. Documentation Updates

#### 10.1 User Documentation
- Update workflow creation guide
- Add parameter definition examples
- CLI usage documentation with parameters

#### 10.2 Developer Documentation
- Parameter parsing implementation
- Template integration details
- Migration guide for existing workflows

### 11. Success Criteria

- [ ] Workflow parameters defined in frontmatter like prompts
- [ ] CLI accepts parameters as named switches
- [ ] Interactive prompting for missing parameters
- [ ] Parameter validation and error handling
- [ ] Backward compatibility maintained during transition
- [ ] All existing workflows migrated to new format
- [ ] Documentation updated and examples provided
- [ ] User experience identical to prompt parameters

## Conclusion

This specification ensures workflow parameters provide the same level of functionality and user experience as prompt parameters. By implementing consistent parameter handling across the system, users will have a unified and predictable interface for both prompts and workflows.