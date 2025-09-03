# Integration Examples

Real-world examples of integrating SwissArmyHammer with development tools, CI/CD systems, and workflows.

## IDE Integration

### VS Code Integration

#### Task Configuration

**File**: `.vscode/tasks.json`
```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Test Prompt",
      "type": "shell",
      "command": "sah",
      "args": [
        "prompt",
        "test",
        "${input:promptName}",
        "--var",
        "file=${file}",
        "--var", 
        "language=${input:language}"
      ],
      "group": "test",
      "presentation": {
        "echo": true,
        "reveal": "always",
        "focus": false,
        "panel": "new"
      },
      "problemMatcher": []
    },
    {
      "label": "Run Workflow",
      "type": "shell",
      "command": "sah",
      "args": [
        "flow",
        "run",
        "${input:workflowName}",
        "--var",
        "file=${file}"
      ],
      "group": "build"
    },
    {
      "label": "Create Issue from Selection",
      "type": "shell",
      "command": "sah",
      "args": [
        "issue",
        "create",
        "--name",
        "${input:issueName}",
        "--content",
        "${selectedText}"
      ],
      "group": "build"
    }
  ],
  "inputs": [
    {
      "id": "promptName",
      "description": "Prompt name",
      "type": "promptString"
    },
    {
      "id": "workflowName", 
      "description": "Workflow name",
      "type": "promptString"
    },
    {
      "id": "language",
      "description": "Programming language",
      "type": "pickString",
      "options": ["rust", "python", "javascript", "typescript", "go"]
    },
    {
      "id": "issueName",
      "description": "Issue name",
      "type": "promptString"
    }
  ]
}
```

#### Keybindings

**File**: `.vscode/keybindings.json`
```json
[
  {
    "key": "ctrl+shift+p t",
    "command": "workbench.action.tasks.runTask",
    "args": "Test Prompt"
  },
  {
    "key": "ctrl+shift+p w",
    "command": "workbench.action.tasks.runTask", 
    "args": "Run Workflow"
  },
  {
    "key": "ctrl+shift+p i",
    "command": "workbench.action.tasks.runTask",
    "args": "Create Issue from Selection"
  }
]
```

### Neovim Integration

**File**: `~/.config/nvim/lua/sah.lua`
```lua
local M = {}

-- Test current prompt
function M.test_prompt()
  local prompt_name = vim.fn.input("Prompt name: ")
  local current_file = vim.fn.expand("%")
  local filetype = vim.bo.filetype
  
  local cmd = string.format(
    "sah prompt test %s --var file=%s --var language=%s",
    prompt_name, current_file, filetype
  )
  
  vim.cmd("split | terminal " .. cmd)
end

-- Create issue from visual selection
function M.create_issue()
  local issue_name = vim.fn.input("Issue name: ")
  local selected_text = vim.fn.getreg('"')
  
  local cmd = string.format(
    "sah issue create --name '%s' --content '%s'",
    issue_name, selected_text
  )
  
  vim.fn.system(cmd)
  print("Issue created: " .. issue_name)
end

-- Search semantic code
function M.semantic_search()
  local query = vim.fn.input("Search query: ")
  local cmd = string.format("sah search query '%s' --format json", query)
  local result = vim.fn.system(cmd)
  
  -- Parse and display results
  local results = vim.fn.json_decode(result)
  vim.cmd("split")
  vim.api.nvim_buf_set_lines(0, 0, -1, false, vim.split(result, "\n"))
end

return M
```

**File**: `~/.config/nvim/init.lua`
```lua
local sah = require('sah')

-- Key mappings
vim.keymap.set('n', '<leader>pt', sah.test_prompt, { desc = 'Test SwissArmyHammer prompt' })
vim.keymap.set('v', '<leader>ic', sah.create_issue, { desc = 'Create issue from selection' })
vim.keymap.set('n', '<leader>ss', sah.semantic_search, { desc = 'Semantic code search' })
```

## Git Integration

### Git Hooks

#### Pre-commit Hook

**File**: `.git/hooks/pre-commit`
```bash
#!/bin/bash

set -e

echo "🔨 Running SwissArmyHammer pre-commit checks..."

# Validate all prompts and workflows
sah validate --strict --format json > validation-results.json

if [ $? -ne 0 ]; then
    echo "❌ Validation failed. Fix issues before committing."
    cat validation-results.json | jq '.errors[]'
    exit 1
fi

# Run code review on changed files
git diff --cached --name-only --diff-filter=AM | grep -E '\.(rs|py|js|ts)$' > changed-code.txt

if [ -s changed-code.txt ]; then
    echo "📝 Running code review on changed files..."
    
    while IFS= read -r file; do
        if [ -f "$file" ]; then
            sah prompt test code-reviewer \
                --var language="$(file-to-lang.sh "$file")" \
                --var file="$file" \
                --output "review-$file.md" \
                --var focus='["bugs", "security"]'
        fi
    done < changed-code.txt
    
    # Create review issue if problems found
    if grep -q "ERROR\|CRITICAL" review-*.md 2>/dev/null; then
        sah issue create \
            --name "review-$(git rev-parse --short HEAD)" \
            --content "$(cat review-*.md)"
        
        echo "❌ Critical issues found. Review issue created."
        rm -f review-*.md changed-code.txt validation-results.json
        exit 1
    fi
    
    rm -f review-*.md
fi

rm -f changed-code.txt validation-results.json
echo "✅ Pre-commit checks passed!"
```

#### Post-commit Hook

**File**: `.git/hooks/post-commit`
```bash
#!/bin/bash

commit_hash=$(git rev-parse HEAD)
commit_message=$(git log -1 --pretty=%B)
files_changed=$(git diff-tree --no-commit-id --name-only -r HEAD | wc -l)

# Create commit memo
sah memo create \
    --title "Commit $(echo $commit_hash | cut -c1-8)" \
    --content "# Commit $commit_hash

## Message
$commit_message

## Files Changed
$files_changed files modified

## Changes
$(git show --stat HEAD)
"

# Index new files for search
git diff-tree --no-commit-id --name-only -r HEAD | while read file; do
    if [[ "$file" =~ \.(rs|py|js|ts|md)$ ]]; then
        sah search index "$file" >/dev/null 2>&1 &
    fi
done
```

### GitHub Actions Integration

**File**: `.github/workflows/sah-integration.yml`
```yaml
name: SwissArmyHammer Integration

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Install SwissArmyHammer
      run: |
        curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
        sudo mv sah /usr/local/bin/
        sah --version
    
    - name: Validate Configuration
      run: |
        sah validate --strict --format json > validation.json
        if [ $? -ne 0 ]; then
          echo "::error::Validation failed"
          cat validation.json | jq '.errors[]' 
          exit 1
        fi
    
    - name: Upload Validation Results
      uses: actions/upload-artifact@v3
      if: always()
      with:
        name: validation-results
        path: validation.json

  code-review:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
    - uses: actions/checkout@v3
      with:
        fetch-depth: 0
    
    - name: Install SwissArmyHammer
      run: |
        curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
        sudo mv sah /usr/local/bin/
    
    - name: Get Changed Files
      id: changed-files
      run: |
        git diff --name-only origin/main...HEAD | grep -E '\.(rs|py|js|ts)$' > changed-files.txt || echo "No code files changed"
    
    - name: Run Code Review
      if: hashFiles('changed-files.txt') != ''
      run: |
        mkdir -p reviews
        while IFS= read -r file; do
          if [ -f "$file" ]; then
            lang=$(basename "$file" | sed 's/.*\.//')
            sah prompt test code-reviewer \
              --var language="$lang" \
              --var file="$file" \
              --var focus='["bugs", "security", "performance"]' \
              --output "reviews/review-$(basename "$file").md"
          fi
        done < changed-files.txt
    
    - name: Comment PR with Review
      uses: actions/github-script@v6
      if: hashFiles('reviews/*.md') != ''
      with:
        script: |
          const fs = require('fs');
          const path = require('path');
          
          let comment = '## 🤖 SwissArmyHammer Code Review\n\n';
          
          const reviewFiles = fs.readdirSync('reviews').filter(f => f.endsWith('.md'));
          
          for (const file of reviewFiles) {
            const content = fs.readFileSync(path.join('reviews', file), 'utf8');
            comment += `### ${file.replace('review-', '').replace('.md', '')}\n\n`;
            comment += content + '\n\n';
          }
          
          github.rest.issues.createComment({
            issue_number: context.issue.number,
            owner: context.repo.owner,
            repo: context.repo.repo,
            body: comment
          });

  semantic-index:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Install SwissArmyHammer
      run: |
        curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
        sudo mv sah /usr/local/bin/
    
    - name: Index Codebase
      run: |
        sah search index "**/*.{rs,py,js,ts}" --force
    
    - name: Cache Search Index
      uses: actions/cache@v3
      with:
        path: ~/.swissarmyhammer/search.db
        key: search-index-${{ github.sha }}
        restore-keys: |
          search-index-
```

## CI/CD Pipeline Integration

### Jenkins Integration

**File**: `Jenkinsfile`
```groovy
pipeline {
    agent any
    
    environment {
        SAH_HOME = "${WORKSPACE}/.swissarmyhammer"
    }
    
    stages {
        stage('Setup SwissArmyHammer') {
            steps {
                script {
                    sh '''
                        curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
                        chmod +x sah
                        ./sah --version
                    '''
                }
            }
        }
        
        stage('Validate Configuration') {
            steps {
                sh './sah validate --strict --format json > validation.json'
                publishHTML([
                    allowMissing: false,
                    alwaysLinkToLastBuild: false,
                    keepAll: true,
                    reportDir: '.',
                    reportFiles: 'validation.json',
                    reportName: 'Validation Report'
                ])
            }
        }
        
        stage('Code Review') {
            when {
                changeRequest()
            }
            steps {
                script {
                    def changedFiles = sh(
                        script: "git diff --name-only origin/main...HEAD | grep -E '\\.(rs|py|js|ts)\$' || echo ''",
                        returnStdout: true
                    ).trim()
                    
                    if (changedFiles) {
                        changedFiles.split('\n').each { file ->
                            if (file.trim()) {
                                def lang = file.split('\\.').last()
                                sh "./sah prompt test code-reviewer --var language=${lang} --var file=${file} --output review-${file}.md"
                            }
                        }
                        
                        // Archive review reports
                        archiveArtifacts artifacts: 'review-*.md', allowEmptyArchive: true
                    }
                }
            }
        }
        
        stage('Workflow Execution') {
            parallel {
                stage('Build Workflow') {
                    steps {
                        sh './sah flow run build-workflow --var environment=${BRANCH_NAME}'
                    }
                }
                stage('Test Workflow') {
                    steps {
                        sh './sah flow run test-workflow --var coverage_threshold=80'
                    }
                }
            }
        }
        
        stage('Semantic Indexing') {
            steps {
                sh './sah search index "**/*.{rs,py,js,ts}" --force'
                archiveArtifacts artifacts: '.swissarmyhammer/search.db', allowEmptyArchive: true
            }
        }
        
        stage('Issue Management') {
            when {
                anyOf {
                    branch 'main'
                    branch 'develop'
                }
            }
            steps {
                script {
                    // Create deployment issue
                    sh """
                        ./sah issue create \\
                            --name 'deploy-${BUILD_NUMBER}' \\
                            --content '# Deployment ${BUILD_NUMBER}
                            
## Build Info
- Branch: ${BRANCH_NAME}
- Commit: ${GIT_COMMIT}
- Build: ${BUILD_NUMBER}
- Timestamp: \$(date)

## Changes
\$(git log --oneline \${GIT_PREVIOUS_COMMIT}..\${GIT_COMMIT})
'
                    """
                }
            }
        }
    }
    
    post {
        always {
            // Create build memo
            sh """
                ./sah memo create \\
                    --title 'Build ${BUILD_NUMBER} - ${BRANCH_NAME}' \\
                    --content '# Build Report
                    
## Status
Status: ${currentBuild.currentResult}

## Duration  
Duration: ${currentBuild.durationString}

## Environment
- Node: ${NODE_NAME}
- Workspace: ${WORKSPACE}
- Branch: ${BRANCH_NAME}
- Commit: ${GIT_COMMIT}

## Test Results
\$(cat test-results.txt 2>/dev/null || echo "No test results")
'
            """
        }
        failure {
            sh """
                ./sah issue create \\
                    --name 'build-failure-${BUILD_NUMBER}' \\
                    --content '# Build Failure ${BUILD_NUMBER}
                    
Build failed on ${BRANCH_NAME} at ${BUILD_TIMESTAMP}

## Error Log
\$(tail -50 ${WORKSPACE}/build.log)

## Investigation Steps
- [ ] Check build logs
- [ ] Verify dependencies  
- [ ] Test locally
- [ ] Check recent changes
'
            """
        }
    }
}
```

### GitLab CI Integration

**File**: `.gitlab-ci.yml`
```yaml
variables:
  SAH_VERSION: "latest"
  SAH_HOME: "$CI_PROJECT_DIR/.swissarmyhammer"

stages:
  - setup
  - validate
  - review
  - build
  - test
  - deploy
  - cleanup

install_sah:
  stage: setup
  script:
    - curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
    - chmod +x sah
    - ./sah --version
  artifacts:
    paths:
      - sah
    expire_in: 1 hour

validate_config:
  stage: validate
  dependencies:
    - install_sah
  script:
    - ./sah validate --strict --format json | tee validation.json
  artifacts:
    reports:
      junit: validation.json
    paths:
      - validation.json
    expire_in: 1 week

code_review:
  stage: review
  dependencies:
    - install_sah
  only:
    - merge_requests
  script:
    - git diff --name-only $CI_MERGE_REQUEST_TARGET_BRANCH_SHA...HEAD | grep -E '\.(rs|py|js|ts)$' > changed-files.txt || echo "No code files changed"
    - |
      if [ -s changed-files.txt ]; then
        mkdir -p reviews
        while IFS= read -r file; do
          if [ -f "$file" ]; then
            lang=$(basename "$file" | sed 's/.*\.//')
            ./sah prompt test code-reviewer \
              --var language="$lang" \
              --var file="$file" \
              --var focus='["bugs", "security"]' \
              --output "reviews/review-$(basename "$file").md"
          fi
        done < changed-files.txt
        
        # Post review as MR comment
        if ls reviews/*.md 1> /dev/null 2>&1; then
          echo "## 🤖 SwissArmyHammer Code Review" > mr-comment.md
          cat reviews/*.md >> mr-comment.md
          
          curl -X POST \
            -H "PRIVATE-TOKEN: $CI_JOB_TOKEN" \
            -H "Content-Type: application/json" \
            -d "{\"body\": \"$(cat mr-comment.md | sed 's/"/\\"/g' | tr '\n' ' ')\"}" \
            "$CI_API_V4_URL/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"
        fi
      fi
  artifacts:
    paths:
      - reviews/
    expire_in: 1 week

run_workflows:
  stage: build
  dependencies:
    - install_sah
  parallel:
    matrix:
      - WORKFLOW: ["build-workflow", "test-workflow", "security-workflow"]
  script:
    - ./sah flow run $WORKFLOW --var environment=$CI_COMMIT_REF_NAME
  artifacts:
    paths:
      - workflow-*.log
    expire_in: 1 day

semantic_indexing:
  stage: build
  dependencies:
    - install_sah
  script:
    - ./sah search index "**/*.{rs,py,js,ts}" --force
  artifacts:
    paths:
      - .swissarmyhammer/search.db
    expire_in: 1 week
  cache:
    key: semantic-index-$CI_COMMIT_SHA
    paths:
      - .swissarmyhammer/search.db

create_deployment_issue:
  stage: deploy
  dependencies:
    - install_sah
  only:
    - main
    - develop
  script:
    - |
      ./sah issue create \
        --name "deploy-$CI_PIPELINE_ID" \
        --content "# Deployment $CI_PIPELINE_ID

## Pipeline Info
- Branch: $CI_COMMIT_REF_NAME  
- Commit: $CI_COMMIT_SHA
- Pipeline: $CI_PIPELINE_ID
- Timestamp: $(date)

## Changes
$(git log --oneline $CI_COMMIT_BEFORE_SHA..$CI_COMMIT_SHA)

## Deployment Checklist
- [ ] Pre-deployment tests pass
- [ ] Database migrations applied
- [ ] Configuration updated
- [ ] Health checks pass
- [ ] Monitoring alerts configured
"

create_build_memo:
  stage: cleanup
  dependencies:
    - install_sah
  when: always
  script:
    - |
      ./sah memo create \
        --title "Pipeline $CI_PIPELINE_ID - $CI_COMMIT_REF_NAME" \
        --content "# Pipeline Report

## Status
Status: $CI_JOB_STATUS

## Timing
- Started: $CI_PIPELINE_CREATED_AT
- Duration: $((CI_PIPELINE_CREATED_AT - $(date +%s))) seconds

## Environment  
- Runner: $CI_RUNNER_DESCRIPTION
- Branch: $CI_COMMIT_REF_NAME
- Commit: $CI_COMMIT_SHA

## Artifacts
$(find . -name '*.log' -o -name '*.json' -o -name '*.md' | head -10)
"
```

## Docker Integration

### Multi-stage Dockerfile with SwissArmyHammer

```dockerfile
# Build stage
FROM rust:1.70 as builder

# Install SwissArmyHammer
RUN curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz && \
    mv sah /usr/local/bin/

# Copy source
WORKDIR /app
COPY . .

# Validate configuration
RUN sah validate --strict

# Run pre-build workflow
RUN sah flow run pre-build-workflow --var environment=container

# Build application
RUN cargo build --release

# Runtime stage  
FROM ubuntu:22.04

# Install SwissArmyHammer for runtime
RUN apt-get update && apt-get install -y curl && \
    curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz && \
    mv sah /usr/local/bin/ && \
    rm -rf /var/lib/apt/lists/*

# Copy application
COPY --from=builder /app/target/release/myapp /usr/local/bin/

# Copy SwissArmyHammer configuration
COPY --from=builder /app/.swissarmyhammer /opt/swissarmyhammer

# Health check using SwissArmyHammer
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD sah prompt test health-check --var service=myapp || exit 1

ENTRYPOINT ["myapp"]
```

### Docker Compose with SwissArmyHammer

```yaml
version: '3.8'

services:
  app:
    build: .
    environment:
      - SAH_HOME=/opt/swissarmyhammer
      - SAH_LOG_LEVEL=info
    volumes:
      - ./workflows:/opt/swissarmyhammer/workflows
      - sah-data:/opt/swissarmyhammer/data
    healthcheck:
      test: ["CMD", "sah", "prompt", "test", "health-check", "--var", "service=app"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  sah-server:
    image: swissarmyhammer/swissarmyhammer:latest
    command: ["sah", "serve", "--port", "8080"]
    ports:
      - "8080:8080"
    volumes:
      - ./prompts:/app/prompts:ro
      - ./workflows:/app/workflows:ro
      - sah-data:/app/data
    environment:
      - SAH_LOG_LEVEL=debug
      - SAH_MCP_TIMEOUT=60000

  workflow-runner:
    image: swissarmyhammer/swissarmyhammer:latest
    command: ["sah", "flow", "run", "monitoring-workflow", "--var", "interval=60"]
    depends_on:
      - app
    volumes:
      - ./workflows:/app/workflows:ro
      - sah-data:/app/data
    restart: unless-stopped

volumes:
  sah-data:
```

## Kubernetes Integration

### SwissArmyHammer as a Service

```yaml
# sah-configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: sah-config
data:
  sah.toml: |
    [general]
    auto_reload = true
    
    [logging]
    level = "info"
    format = "json"
    
    [mcp]
    enable_tools = ["issues", "memoranda", "search"]
    timeout_ms = 30000

---
# sah-deployment.yaml  
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sah-server
spec:
  replicas: 2
  selector:
    matchLabels:
      app: sah-server
  template:
    metadata:
      labels:
        app: sah-server
    spec:
      containers:
      - name: sah-server
        image: swissarmyhammer/swissarmyhammer:latest
        command: ["sah", "serve", "--port", "8080"]
        ports:
        - containerPort: 8080
        env:
        - name: SAH_HOME
          value: "/app/sah"
        - name: SAH_LOG_LEVEL
          value: "info"
        volumeMounts:
        - name: config
          mountPath: /app/sah/sah.toml
          subPath: sah.toml
        - name: prompts
          mountPath: /app/sah/prompts
        - name: workflows  
          mountPath: /app/sah/workflows
        - name: data
          mountPath: /app/sah/data
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: sah-config
      - name: prompts
        configMap:
          name: sah-prompts
      - name: workflows
        configMap:
          name: sah-workflows
      - name: data
        persistentVolumeClaim:
          claimName: sah-data

---
# sah-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: sah-server
spec:
  selector:
    app: sah-server
  ports:
  - protocol: TCP
    port: 80
    targetPort: 8080
  type: ClusterIP
```

### CronJob for Automated Workflows

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: sah-maintenance
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: sah-maintenance
            image: swissarmyhammer/swissarmyhammer:latest
            command:
            - /bin/bash
            - -c
            - |
              # Run maintenance workflow
              sah flow run maintenance-workflow --var environment=production
              
              # Cleanup old issues
              sah issue list --status complete --format json | \
                jq -r '.[] | select(.created < (now - 86400*30)) | .name' | \
                xargs -I {} sah issue delete {}
              
              # Update search index
              sah search index "**/*.{rs,py,js,ts}" --force
            env:
            - name: SAH_HOME
              value: "/app/sah"
            volumeMounts:
            - name: sah-data
              mountPath: /app/sah/data
          restartPolicy: OnFailure
          volumes:
          - name: sah-data
            persistentVolumeClaim:
              claimName: sah-data
```

These integration examples demonstrate how SwissArmyHammer can be seamlessly incorporated into existing development workflows, providing AI-powered automation and analysis capabilities across the entire software development lifecycle.