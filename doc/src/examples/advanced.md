# Advanced Examples

Complex, real-world examples demonstrating advanced SwissArmyHammer features and patterns.

## Advanced Prompts

### Multi-Language Code Analyzer

A sophisticated prompt that analyzes code across multiple languages:

**File**: `~/.swissarmyhammer/prompts/multi-analyzer.md`
```markdown
---
title: Multi-Language Code Analyzer
description: Analyze code across different languages with unified reporting
version: "2.0"
arguments:
  - name: files
    description: Files to analyze (JSON array of {path, language, content})
    type: array
    required: true
  - name: analysis_type
    description: Type of analysis to perform
    choices: ["security", "performance", "architecture", "comprehensive"]
    default: "comprehensive"
  - name: output_format
    description: Output format for results
    choices: ["markdown", "json", "html"]
    default: "markdown"
  - name: severity_threshold
    description: Minimum severity to report
    choices: ["info", "warning", "error", "critical"]
    default: "warning"
---

# Multi-Language Code Analysis Report

{% assign total_files = files | size %}
{% assign languages = files | map: "language" | uniq %}

## Overview
- **Files Analyzed**: {{total_files}}
- **Languages**: {{languages | join: ", "}}
- **Analysis Type**: {{analysis_type | capitalize}}
- **Threshold**: {{severity_threshold | capitalize}}

{% for language in languages %}
{% assign lang_files = files | where: "language", language %}
## {{language | capitalize}} Analysis ({{lang_files | size}} files)

{% for file in lang_files %}
### {{file.path}}

{% case analysis_type %}
{% when "security" %}
Perform security analysis for {{language}}:
- Check for injection vulnerabilities
- Validate input sanitization
- Review authentication/authorization
- Check for hardcoded secrets
{% when "performance" %}
Perform performance analysis for {{language}}:
- Identify algorithmic inefficiencies
- Check memory usage patterns
- Review I/O operations
- Analyze concurrency issues
{% when "architecture" %}
Perform architectural analysis for {{language}}:
- Review design patterns usage
- Check separation of concerns
- Analyze dependencies
- Review error handling strategy
{% else %}
Perform comprehensive analysis for {{language}}:
- Security vulnerabilities
- Performance bottlenecks
- Architectural issues
- Code quality concerns
{% endcase %}

**Code:**
```{{language}}
{{file.content}}
```

{% endfor %}
{% endfor %}

## Analysis Instructions

For each file, provide:

1. **Summary** - Overall code quality assessment
2. **Issues Found** - Categorized by severity ({{severity_threshold}}+)
3. **Recommendations** - Specific improvement suggestions
4. **Code Examples** - Corrected code snippets where applicable

{% if output_format == "json" %}
Format the response as valid JSON with this structure:
```json
{
  "summary": "Overall assessment",
  "files": [
    {
      "path": "file_path",
      "language": "language",
      "issues": [
        {
          "severity": "error|warning|info",
          "category": "security|performance|architecture|quality",
          "line": 42,
          "message": "Description of issue",
          "recommendation": "How to fix it"
        }
      ]
    }
  ]
}
```
{% elsif output_format == "html" %}
Format as clean HTML with proper styling and navigation.
{% else %}
Use clear markdown formatting with proper headers and code blocks.
{% endif %}
```

### Intelligent Test Generator

Generate comprehensive test suites based on code analysis:

**File**: `~/.swissarmyhammer/prompts/test-generator.md`
```markdown
---
title: Intelligent Test Generator
description: Generate comprehensive test suites with edge cases
arguments:
  - name: code
    description: Source code to test
    required: true
  - name: language
    description: Programming language
    required: true
  - name: test_framework
    description: Testing framework to use
    required: false
  - name: coverage_goal
    description: Target code coverage percentage
    type: number
    default: 90
  - name: include_integration
    description: Include integration tests
    type: boolean
    default: true
  - name: include_performance
    description: Include performance tests
    type: boolean
    default: false
---

# Test Suite Generation for {{language | capitalize}}

## Source Code Analysis

```{{language}}
{{code}}
```

## Test Requirements
- **Framework**: {% if test_framework %}{{test_framework}}{% else %}Standard {{language}} testing{% endif %}
- **Coverage Goal**: {{coverage_goal}}%
- **Integration Tests**: {% if include_integration %}Yes{% else %}No{% endif %}
- **Performance Tests**: {% if include_performance %}Yes{% else %}No{% endif %}

## Generate Tests

Please create a comprehensive test suite including:

### 1. Unit Tests
- Test all public functions/methods
- Cover happy path scenarios
- Test edge cases and boundary conditions
- Test error conditions and exception handling
- Validate input validation

### 2. Property-Based Tests (if applicable)
- Generate tests for invariant properties
- Test with random inputs
- Verify mathematical properties

{% if include_integration %}
### 3. Integration Tests
- Test component interactions
- Test external API integrations
- Test database operations
- Test file system operations
{% endif %}

{% if include_performance %}
### 4. Performance Tests
- Benchmark critical functions
- Test memory usage
- Test with large datasets
- Measure execution time
{% endif %}

### 5. Test Data & Fixtures
- Create realistic test data
- Mock external dependencies
- Set up test databases/files

## Coverage Analysis
Aim for {{coverage_goal}}% coverage by testing:
- All code paths
- Conditional branches
- Loop iterations
- Exception handlers

{% case language %}
{% when "rust" %}
Use `cargo test` conventions with:
- `#[test]` attributes
- `assert_eq!`, `assert!` macros
- `#[should_panic]` for error cases
- `proptest` for property-based testing
{% when "python" %}
Use pytest conventions with:
- `test_` function prefixes
- `assert` statements
- `@pytest.fixture` for setup
- `@pytest.parametrize` for data-driven tests
{% when "javascript" %}
Use Jest conventions with:
- `describe()` and `it()` blocks
- `expect().toBe()` assertions
- `beforeEach()`/`afterEach()` hooks
- Mock functions for dependencies
{% when "typescript" %}
Use Jest with TypeScript:
- Type-safe test code
- Interface mocking
- Generic test utilities
- Async/await testing patterns
{% endcase %}

Generate complete, runnable test code with clear documentation.
```

## Advanced Workflows

### Complete CI/CD Pipeline

A full-featured continuous integration and deployment workflow:

**File**: `~/.swissarmyhammer/workflows/ci-cd-pipeline.md`
```markdown
---
name: ci-cd-pipeline
description: Complete CI/CD pipeline with quality gates
version: "3.0"
initial_state: validate-pr
max_parallel: 6

variables:
  - name: environment
    description: Target deployment environment
    choices: ["dev", "staging", "prod"]
    default: "dev"
  - name: skip_tests
    description: Skip test execution (emergency deploys only)
    type: boolean
    default: false
  - name: deployment_strategy
    description: Deployment strategy
    choices: ["rolling", "blue-green", "canary"]
    default: "rolling"
  - name: auto_rollback
    description: Enable automatic rollback on failure
    type: boolean
    default: true

resources:
  - name: test-db
    type: docker-container
    image: postgres:13
    env:
      POSTGRES_DB: testdb
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    cleanup: true
---

# CI/CD Pipeline Workflow

## validate-pr
**Description**: Validate pull request and setup

**Actions:**
- shell: `git fetch origin main`
- shell: `git diff --name-only origin/main...HEAD > changed-files.txt`
- conditional: Check if critical files changed
  condition: file_contains("changed-files.txt", "Cargo.toml|package.json|requirements.txt")
  true_state: dependency-check
- conditional: Security scan needed
  condition: file_contains("changed-files.txt", ".rs|.py|.js|.ts")
  true_action: shell: `security-scan.sh`

**Transitions:**
- If security issues found → security-review
- If dependencies changed → dependency-check
- Always → code-quality

## dependency-check
**Description**: Analyze dependency changes

**Actions:**
- shell: `cargo audit` (parallel, if Rust)
- shell: `npm audit` (parallel, if Node.js)
- shell: `safety check` (parallel, if Python)
- prompt: multi-analyzer files="$(cat changed-files.txt | head -10)" analysis_type="security"

**Transitions:**
- If vulnerabilities found → security-review
- Always → code-quality

## code-quality
**Description**: Run code quality checks

**Actions:**
- shell: `cargo fmt --check` (parallel, timeout: 30s)
- shell: `cargo clippy -- -D warnings` (parallel, timeout: 120s)
- shell: `eslint --ext .js,.ts .` (parallel, timeout: 60s)
- prompt: code-reviewer language="rust" code="$(git diff origin/main...HEAD)"

**Transitions:**
- If quality issues found → quality-review
- If skip_tests == true → build
- Always → test-suite

## test-suite
**Description**: Execute comprehensive test suite

**Actions:**
- fork: unit-tests
  actions:
    - shell: `cargo test --lib`
    - shell: `npm test -- --coverage`
    - shell: `pytest tests/unit/`
- fork: integration-tests
  actions:
    - shell: `cargo test --test integration`
    - shell: `npm run test:integration`
- fork: e2e-tests
  actions:
    - shell: `npm run test:e2e`
    - shell: `pytest tests/e2e/`
  condition: environment != "dev"

**Transitions:**
- If any tests fail → test-failure
- When all forks complete → build

## build
**Description**: Build artifacts for deployment

**Actions:**
- shell: `docker build -t app:{{git.commit}} .`
- shell: `docker tag app:{{git.commit}} app:{{environment}}-latest`
- conditional: Multi-arch build
  condition: environment == "prod"
  true_action: shell: `docker buildx build --platform linux/amd64,linux/arm64 -t app:{{git.commit}} .`

**Transitions:**
- On success → security-scan
- On failure → build-failure

## security-scan
**Description**: Security scanning of built artifacts

**Actions:**
- shell: `trivy image app:{{git.commit}}`
- shell: `docker run --rm app:{{git.commit}} security-check.sh`
- prompt: multi-analyzer analysis_type="security" severity_threshold="error"

**Transitions:**
- If critical vulnerabilities → security-review
- Always → deploy-{{environment}}

## deploy-dev
**Description**: Deploy to development environment

**Actions:**
- shell: `kubectl config use-context dev`
- shell: `helm upgrade --install app ./charts/app --set image.tag={{git.commit}}`
- shell: `kubectl wait --for=condition=available --timeout=300s deployment/app`

**Transitions:**
- On success → smoke-tests
- On failure → deployment-failure

## deploy-staging
**Description**: Deploy to staging environment

**Actions:**
- shell: `kubectl config use-context staging`
- conditional: Blue-green deployment
  condition: deployment_strategy == "blue-green"
  true_workflow: blue-green-deploy
- conditional: Canary deployment
  condition: deployment_strategy == "canary"
  true_workflow: canary-deploy
- shell: `helm upgrade --install app ./charts/app --set image.tag={{git.commit}}`

**Transitions:**
- On success → staging-tests
- On failure → rollback-staging

## deploy-prod
**Description**: Deploy to production environment

**Actions:**
- shell: `kubectl config use-context prod`
- shell: `helm upgrade --install app ./charts/app --set image.tag={{git.commit}} --wait --timeout=10m`
- shell: `kubectl annotate deployment app deployment.kubernetes.io/revision={{git.commit}}`

**Transitions:**
- On success → production-verification
- On failure → rollback-production

## smoke-tests
**Description**: Run smoke tests against deployed application

**Actions:**
- wait: 30s  # Allow deployment to stabilize
- shell: `curl -f http://app-{{environment}}.local/health`
- shell: `npm run test:smoke -- --env={{environment}}`

**Transitions:**
- On success → complete
- On failure → deployment-failure

## staging-tests
**Description**: Run full test suite against staging

**Actions:**
- shell: `npm run test:api -- --env=staging`
- shell: `npm run test:performance -- --env=staging`
- prompt: test-generator code="$(cat src/main.rs)" include_performance=true

**Transitions:**
- On success → staging-approval
- On failure → rollback-staging

## production-verification
**Description**: Verify production deployment

**Actions:**
- shell: `kubectl get pods -l app=myapp`
- shell: `curl -f https://api.myapp.com/health`
- shell: `npm run test:production-smoke`
- wait: 300s  # Monitor for 5 minutes
- shell: `check-error-rates.sh`

**Transitions:**
- On success → complete
- If auto_rollback && errors detected → rollback-production
- On failure → production-incident

## Error States

## test-failure
**Description**: Handle test failures

**Actions:**
- prompt: helper task="analyzing test failures" detail_level="comprehensive"
- shell: `generate-test-report.sh > test-report.html`
- issue: create
  name: "test-failure-{{git.commit | slice: 0, 8}}"
  content: "Test failures in {{git.branch}}: $(cat test-failures.log)"

**Transitions:**
- Always → failed

## build-failure
**Description**: Handle build failures

**Actions:**
- prompt: multi-analyzer analysis_type="architecture" files="$(find . -name '*.rs' -o -name '*.toml')"
- shell: `docker logs $(docker ps -q) > build-logs.txt`

**Transitions:**
- Always → failed

## security-review
**Description**: Manual security review required

**Actions:**
- prompt: helper task="security review process" detail_level="comprehensive"
- issue: create
  name: "security-review-{{execution.start_time | date: '%Y%m%d-%H%M'}}"
  content: "Security review required for deployment to {{environment}}"

**Transitions:**
- Manual approval → continue-pipeline
- Always → blocked

## rollback-staging
**Description**: Rollback staging deployment

**Actions:**
- shell: `helm rollback app --namespace staging`
- shell: `kubectl wait --for=condition=available --timeout=300s deployment/app -n staging`

**Transitions:**
- Always → failed

## rollback-production
**Description**: Emergency production rollback

**Actions:**
- shell: `helm rollback app --namespace production`
- shell: `kubectl wait --for=condition=available --timeout=300s deployment/app -n production`
- shell: `send-alert.sh "Production rollback executed for {{git.commit}}"`

**Transitions:**
- Always → production-incident

## production-incident
**Description**: Handle production incidents

**Actions:**
- shell: `create-incident.sh --severity=high --title="Deployment failure {{git.commit}}"`
- prompt: helper task="production incident response" detail_level="comprehensive"
- issue: create
  name: "prod-incident-{{execution.start_time | date: '%Y%m%d-%H%M'}}"
  content: "Production incident during deployment of {{git.commit}} to {{environment}}"

**Transitions:**
- Always → failed

## staging-approval
**Description**: Wait for staging approval

**Actions:**
- prompt: helper task="staging deployment approval" detail_level="brief"
- wait: until approval_received("staging")

**Transitions:**
- On approval → deploy-prod
- On rejection → failed

## complete
**Description**: Pipeline completed successfully

**Actions:**
- shell: `update-deployment-status.sh --status=success --commit={{git.commit}}`
- memo: create
  title: "Successful deployment {{git.commit | slice: 0, 8}}"
  content: "Deployed {{git.commit}} to {{environment}} successfully at {{execution.start_time}}"

## failed
**Description**: Pipeline failed

**Actions:**
- shell: `update-deployment-status.sh --status=failed --commit={{git.commit}}`
- shell: `cleanup-resources.sh`
```

### Microservices Orchestration

Complex workflow for managing microservices deployments:

**File**: `~/.swissarmyhammer/workflows/microservices-deploy.md`
```markdown
---
name: microservices-deploy
description: Orchestrate deployment of multiple microservices with dependencies
initial_state: dependency-analysis
variables:
  - name: services
    description: Services to deploy (JSON array)
    type: array
    required: true
  - name: environment
    description: Target environment
    choices: ["dev", "staging", "prod"]
    default: "dev"
  - name: strategy
    description: Deployment strategy
    choices: ["sequential", "parallel", "dependency-order"]
    default: "dependency-order"
---

## dependency-analysis
**Description**: Analyze service dependencies and create deployment order

**Actions:**
- shell: `analyze-dependencies.py {{services | join: " "}} > dependency-graph.json`
- prompt: multi-analyzer files="$(cat dependency-graph.json)" analysis_type="architecture"

**Transitions:**
- If strategy == "sequential" → deploy-sequential
- If strategy == "parallel" → deploy-parallel
- Always → deploy-dependency-order

## deploy-dependency-order
**Description**: Deploy services in dependency order

**Actions:**
- loop: Deploy each service layer
  items: "{{dependency_layers}}"
  state: deploy-service-layer
  parallel: false

**Transitions:**
- When loop complete → integration-tests
- On any failure → rollback-all

## deploy-service-layer
**Description**: Deploy all services in current dependency layer

**Actions:**
- fork: Deploy services in parallel
  items: "{{current_layer.services}}"
  template: |
    shell: `helm upgrade --install {{item.name}} ./charts/{{item.name}} --set image.tag={{item.version}} --namespace {{environment}}`
    wait: until service_healthy("{{item.name}}")

**Transitions:**
- When all forks complete → next-layer
- On any failure → layer-failure

## integration-tests
**Description**: Run integration tests across all services

**Actions:**
- shell: `run-integration-tests.sh {{services | join: " "}} --env={{environment}}`
- prompt: test-generator code="$(cat integration-tests/*.py)" include_integration=true

**Transitions:**
- On success → service-mesh-config
- On failure → integration-failure

## service-mesh-config
**Description**: Configure service mesh and networking

**Actions:**
- shell: `kubectl apply -f service-mesh/{{environment}}/ --recursive`
- shell: `istioctl proxy-config cluster {{services | first}}-pod`
- wait: 60s  # Allow mesh configuration to propagate

**Transitions:**
- Always → end-to-end-tests

## end-to-end-tests
**Description**: Run end-to-end tests across the entire system

**Actions:**
- shell: `npm run test:e2e -- --env={{environment}} --services="{{services | join: ','}}"`
- wait: 120s  # Allow system to stabilize

**Transitions:**
- On success → complete
- On failure → system-failure
```

## Advanced Template Patterns

### Conditional Logic and Loops

```liquid
{% comment %} Complex conditional logic {% endcomment %}
{% assign lang = language | default: "unknown" | downcase %}
{% assign is_compiled = false %}
{% assign is_interpreted = false %}
{% assign has_package_manager = false %}

{% case lang %}
  {% when "rust", "go", "c", "cpp" %}
    {% assign is_compiled = true %}
  {% when "python", "javascript", "ruby" %}
    {% assign is_interpreted = true %}
{% endcase %}

{% if lang == "rust" or lang == "javascript" or lang == "python" %}
  {% assign has_package_manager = true %}
{% endif %}

## {{lang | capitalize}} Project Analysis

{% if is_compiled %}
### Compilation Requirements
- Build system: {% if lang == "rust" %}Cargo{% elsif lang == "go" %}Go modules{% else %}Make/CMake{% endif %}
- Optimization flags needed for production builds
{% endif %}

{% if has_package_manager %}
### Dependency Management
{% case lang %}
{% when "rust" %}
- Package manager: Cargo
- Manifest file: `Cargo.toml`
- Lock file: `Cargo.lock`
{% when "javascript" %}
- Package manager: npm/yarn/pnpm
- Manifest file: `package.json`
- Lock file: `package-lock.json`/`yarn.lock`
{% when "python" %}
- Package manager: pip/poetry/conda
- Manifest files: `requirements.txt`, `pyproject.toml`
- Lock file: `poetry.lock`
{% endcase %}
{% endif %}

{% comment %} Loop through complex data structures {% endcomment %}
{% assign grouped_issues = issues | group_by: "severity" %}
{% for group in grouped_issues %}
### {{group.name | capitalize}} Issues ({{group.items | size}})
  {% for issue in group.items %}
    {% assign icon = "ℹ️" %}
    {% if issue.severity == "error" %}{% assign icon = "❌" %}{% endif %}
    {% if issue.severity == "warning" %}{% assign icon = "⚠️" %}{% endif %}
- {{icon}} **{{issue.title}}** (Line {{issue.line}})
  {{issue.description}}
  {% if issue.fix %}
  **Fix**: {{issue.fix}}
  {% endif %}
  {% endfor %}
{% endfor %}
```

### Advanced Filter Usage

```liquid
{% comment %} Custom filters and complex transformations {% endcomment %}
{% assign functions = code_analysis.functions | sort: "complexity" | reverse %}
{% assign high_complexity = functions | where: "complexity", "> 10" %}

## Complexity Analysis

### High Complexity Functions ({{high_complexity | size}})
{% for func in high_complexity %}
- **{{func.name}}** ({{func.complexity}} complexity)
  - Lines: {{func.start_line}}-{{func.end_line}}
  - Parameters: {{func.parameters | size}}
  - Return type: {{func.return_type | default: "void"}}
  {% if func.issues %}
  - Issues: {{func.issues | map: "type" | uniq | join: ", "}}
  {% endif %}
{% endfor %}

### Refactoring Suggestions
{% assign refactor_candidates = high_complexity | slice: 0, 3 %}
{% for func in refactor_candidates %}
{{forloop.index}}. **{{func.name}}**
   - Current complexity: {{func.complexity}}
   - Suggested approach: {{func.complexity | complexity_strategy}}
   - Estimated effort: {{func.lines | lines_to_effort}} hours
{% endfor %}

### Code Metrics Summary
- Total functions: {{functions | size}}
- Average complexity: {{functions | map: "complexity" | sum | divided_by: functions.size | round: 1}}
- High complexity (>10): {{high_complexity | size}} ({{high_complexity | size | times: 100.0 | divided_by: functions.size | round: 1}}%)
- Maintainability index: {{code_analysis.maintainability | round: 1}}/100
```

These advanced examples demonstrate sophisticated patterns for complex real-world scenarios, showing how SwissArmyHammer can handle enterprise-level automation and analysis tasks.