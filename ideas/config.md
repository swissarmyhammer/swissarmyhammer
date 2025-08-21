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

Move the config file to the new search locations. Use figment rather than custom parsing. Continue to load the config into the templating system - making sure to preserve a single templating system for prompts and workflows.

The custom env var parsing, types, validation, types, loader should all be eliminated. Strive to use `figment` directly and avoid duplicating or making a lot of code that can be had just by using figment.

### 1. Configuration Precedence Order

Configuration sources should be merged in the following order (later sources override earlier ones):

1. **Default values** (hardcoded in application)
2. **Global config file** (`~/.swissarmyhammer/` directory)
3. **Project config file** (`.swissarmyhammer/` directory in current project)
4. **Environment variables** (with `SAH_` or `SWISSARMYHAMMER_` prefix)
5. **Command line arguments** (highest priority)

### 2. Configuration File Discovery

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
