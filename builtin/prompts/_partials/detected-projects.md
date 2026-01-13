---
title: Detected Projects
description: Automatically detected project types in the current directory
partial: true
---

{% if project_types.size > 0 %}
## Detected Project Types

The following project(s) were automatically detected:

{% for project in project_types %}
### {{ forloop.index }}. {{ project.type | capitalize }} Project

**Location:** `{{ project.path }}`
**Markers:** {{ project.markers | join: ", " }}

{% if project.workspace %}
**Workspace:** Yes ({{ project.workspace.members.size }} members)
{% if project.workspace.members.size > 0 %}  **Members:** {{ project.workspace.members | join: ", " }}
{% endif %}
{% endif %}

{% endfor %}

## Project Guidelines

{% for project_type in unique_project_types %}
  {% case project_type %}
  {% when "Rust" %}
{% include "_partials/project-types/rust" %}
  {% when "NodeJs" %}
{% include "_partials/project-types/nodejs" %}
  {% when "Python" %}
{% include "_partials/project-types/python" %}
  {% when "Go" %}
{% include "_partials/project-types/go" %}
  {% when "JavaMaven" %}
{% include "_partials/project-types/java-maven" %}
  {% when "JavaGradle" %}
{% include "_partials/project-types/java-gradle" %}
  {% when "CSharp" %}
{% include "_partials/project-types/csharp" %}
  {% when "CMake" %}
{% include "_partials/project-types/cmake" %}
  {% when "Makefile" %}
{% include "_partials/project-types/makefile" %}
  {% when "Flutter" %}
{% include "_partials/project-types/flutter" %}
  {% endcase %}
{% endfor %}

## Usage

To test or build this project, use the appropriate commands listed above. **Do NOT** use glob patterns like `*`, `**/*`, or `**/*.ext` - the project detection system has already identified what you need.

{% else %}
## No Projects Detected

No standard project marker files (Cargo.toml, package.json, go.mod, etc.) were found in the current directory tree.
{% endif %}
