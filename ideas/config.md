# SwissArmyHammer Configuration System using Figment

## Overview

The system will support multiple configuration sources with a clear precedence order, multiple file formats, and environment variable integration.

## Current State

Currently, SwissArmyHammer has:
- Custom TOML configuration parsing in `src/sah_config/`
- Limited configuration file discovery
- Basic environment variable support
- Hardcoded configuration paths and formats

## Proposed Design

Move the config file to the new search locations. Use `figment` rather than custom parsing.
Continue to load the config into the templating system - making sure to preserve a single templating system for prompts and workflows.

The `sah_config` module should be eliminated.
The `tom_config` module should be eliminated.
Strive to use `figment` directly and avoid duplicating or making a lot of code that can be had just by using figment.

Note that this config system is not about configuring any of the MCP tools or MCP itself at this time -- it is just a way to provide variables to the rendering subsystem.
Our MCP tools do not currently have configuration.

Separate config into a new crate swissarmyhammer-config.

`merge_config_into_context` and related methods really should be on new TemplateContext object for template `render`. This same context object should be usable by prompts, workflows, and actions. Right now we're using an inferior hashmap as a context object.

Do not cache -- when you need a new TemplateContext, have it read the config and load itself. This allows the user to edit config while running.

There is no need for backward compatibility.

### Configuration Precedence Order

Configuration sources should be merged in the following order (later sources override earlier ones):

1. **Default values** (hardcoded in application)
2. **Global config file** (`~/.swissarmyhammer/` directory)
3. **Project config file** (`.swissarmyhammer/` directory in current project)
4. **Environment variables** (with `SAH_` or `SWISSARMYHAMMER_` prefix)
5. **Command line arguments** (highest priority)

### Configuration File Discovery

#### File Names
Support both short and long form names:
- `sah.{toml,yaml,yml,json}`
- `swissarmyhammer.{toml,yaml,yml,json}`

#### Search Locations
1. **Project SwissArmyHammer Directory**
   ```
   ./.swissarmyhammer/sah.toml
   ./.swissarmyhammer/sah.yaml
   ./.swissarmyhammer/sah.yml
   ./.swissarmyhammer/sah.json
   ./.swissarmyhammer/swissarmyhammer.toml
   ./.swissarmyhammer/swissarmyhammer.yaml
   ./.swissarmyhammer/swissarmyhammer.yml
   ./.swissarmyhammer/swissarmyhammer.json
   ```

2. **User Home SwissArmyHammer Directory**
   ```
   ~/.swissarmyhammer/sah.toml
   ~/.swissarmyhammer/sah.yaml
   ~/.swissarmyhammer/sah.yml
   ~/.swissarmyhammer/sah.json
   ~/.swissarmyhammer/swissarmyhammer.toml
   ~/.swissarmyhammer/swissarmyhammer.yaml
   ~/.swissarmyhammer/swissarmyhammer.yml
   ~/.swissarmyhammer/swissarmyhammer.json
   ```


### Remove the `sah config test` sub command
