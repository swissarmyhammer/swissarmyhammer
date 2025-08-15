# CI/CD Integration Examples

This guide demonstrates how to integrate the shell tool into Continuous Integration and Continuous Deployment pipelines, covering build automation, testing strategies, and deployment workflows.

## GitHub Actions Integration

### Basic Build Pipeline

**Simple CI workflow**:
```yaml
# .github/workflows/ci.yml
name: CI Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Setup SwissArmyHammer
      run: |
        # Install SAH (adjust installation method as needed)
        curl -L https://github.com/your-org/sah-shell/releases/latest/download/sah-linux.tar.gz | tar xz
        sudo mv sah /usr/local/bin/
        
    - name: Run Tests
      run: |
        sah shell -t 900 -e "CI=true" -e "RUST_LOG=info" "cargo test"
        
    - name: Build Release
      run: |
        sah shell -t 1200 -e "CARGO_TERM_COLOR=always" "cargo build --release"
        
    - name: Upload Artifacts
      uses: actions/upload-artifact@v4
      with:
        name: build-artifacts
        path: target/release/
```

### Multi-Language Pipeline

**Complex CI with multiple languages**:
```yaml
# .github/workflows/multi-lang.yml
name: Multi-Language CI

on:
  push:
    branches: [ main ]

jobs:
  test-rust:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Rust Tests and Build
      run: |
        # Format check
        sah shell -C backend "cargo fmt -- --check"
        
        # Linting
        sah shell -C backend "cargo clippy -- -D warnings"
        
        # Tests with coverage
        sah shell -t 900 -C backend -e "CARGO_INCREMENTAL=0" \
          -e "RUSTFLAGS=-Zprofile -Ccodegen-units=1 -Cinline-threshold=0 -Clink-dead-code -Coverflow-checks=off -Cpanic=abort -Zpanic_abort_tests" \
          "cargo test"
        
        # Build release
        sah shell -t 1200 -C backend "cargo build --release"

  test-frontend:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Frontend Tests and Build
      run: |
        # Install dependencies
        sah shell -t 600 -C frontend -e "CI=true" "npm ci"
        
        # Linting
        sah shell -C frontend "npm run lint"
        
        # Type checking
        sah shell -C frontend "npm run type-check"
        
        # Unit tests
        sah shell -t 600 -C frontend -e "CI=true" "npm test -- --coverage"
        
        # Build production
        sah shell -t 900 -C frontend -e "NODE_ENV=production" "npm run build"
        
        # E2E tests
        sah shell -t 1800 -C frontend "npm run test:e2e"

  security-scan:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Security Analysis
      run: |
        # Rust security audit
        sah shell -C backend "cargo audit"
        
        # Node.js security audit  
        sah shell -C frontend "npm audit --audit-level=moderate"
        
        # SAST scanning
        sah shell -t 900 "semgrep --config=auto ."
```

### Docker Integration

**Container-based CI/CD**:
```yaml
# .github/workflows/docker.yml
name: Docker Build and Deploy

on:
  push:
    branches: [ main ]
    tags: [ 'v*' ]

jobs:
  build:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Build Application
      run: |
        # Build within Docker context
        sah shell -t 1800 "docker build -t myapp:build ."
        
        # Run tests in container
        sah shell -t 900 "docker run --rm myapp:build cargo test"
        
        # Extract binary from container
        sah shell "docker create --name temp myapp:build"
        sah shell "docker cp temp:/app/target/release/myapp ./myapp"
        sah shell "docker rm temp"
    
    - name: Build Production Image
      run: |
        # Multi-stage Docker build
        sah shell -t 1200 "docker build -f Dockerfile.prod -t myapp:latest ."
        
        # Security scan of image
        sah shell -t 600 "docker run --rm -v /var/run/docker.sock:/var/run/docker.sock \
          aquasec/trivy image myapp:latest"
    
    - name: Deploy to Staging
      if: github.ref == 'refs/heads/main'
      run: |
        # Deploy to staging environment
        sah shell -t 300 -e "DOCKER_HOST=staging.example.com" \
          "docker service update --image myapp:latest staging_myapp"
```

## GitLab CI/CD Integration

### GitLab Pipeline Configuration

**Complete GitLab CI pipeline**:
```yaml
# .gitlab-ci.yml
stages:
  - test
  - build
  - security
  - deploy

variables:
  RUST_LOG: info
  CARGO_HOME: .cargo
  
cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - .cargo/
    - target/

test:
  stage: test
  image: rust:1.70
  before_script:
    - curl -L https://releases.sah.dev/latest/sah-linux.tar.gz | tar xz
    - mv sah /usr/local/bin/
  script:
    # Format check
    - sah shell "cargo fmt -- --check"
    
    # Linting
    - sah shell "cargo clippy -- -D warnings"
    
    # Unit tests
    - sah shell -t 900 -e "RUST_LOG=debug" "cargo test --verbose"
    
    # Integration tests
    - sah shell -t 1200 "cargo test --test integration_tests"
    
    # Documentation tests  
    - sah shell "cargo test --doc"
  
  coverage: '/^\s*lines:\s*\d+.\d+\%/'
  
  artifacts:
    reports:
      coverage_report:
        coverage_format: cobertura
        path: coverage.xml

build:
  stage: build  
  image: rust:1.70
  script:
    # Optimized release build
    - sah shell -t 1800 -e "RUSTFLAGS=-C target-cpu=native" \
      "cargo build --release"
    
    # Strip binary for smaller size
    - sah shell "strip target/release/myapp"
    
    # Create distribution archive
    - sah shell "tar czf myapp-${CI_COMMIT_SHORT_SHA}.tar.gz \
      -C target/release myapp"
  
  artifacts:
    paths:
      - myapp-*.tar.gz
    expire_in: 1 week

security:
  stage: security
  image: rust:1.70
  allow_failure: true
  script:
    # Security audit
    - sah shell -t 300 "cargo audit"
    
    # Dependency check
    - sah shell "cargo deny check"
    
    # Static analysis
    - sah shell -t 600 "cargo clippy -- -W clippy::all"

deploy-staging:
  stage: deploy
  image: alpine:latest
  only:
    - main
  before_script:
    - apk add --no-cache openssh-client
    - eval $(ssh-agent -s)
    - echo "$SSH_PRIVATE_KEY" | tr -d '\r' | ssh-add -
  script:
    # Deploy to staging server
    - sah shell -t 300 "scp myapp-${CI_COMMIT_SHORT_SHA}.tar.gz \
      user@staging.example.com:/tmp/"
    
    - sah shell -t 120 "ssh user@staging.example.com \
      'cd /opt/myapp && tar xzf /tmp/myapp-${CI_COMMIT_SHORT_SHA}.tar.gz'"
    
    - sah shell -t 60 "ssh user@staging.example.com \
      'systemctl restart myapp'"
    
    # Health check
    - sah shell -t 30 "curl -f http://staging.example.com/health"
```

## Jenkins Integration

### Jenkins Pipeline

**Declarative Jenkins pipeline**:
```groovy
// Jenkinsfile
pipeline {
    agent any
    
    environment {
        RUST_LOG = 'info'
        CARGO_HOME = "${WORKSPACE}/.cargo"
    }
    
    stages {
        stage('Setup') {
            steps {
                script {
                    // Install SAH if not available
                    sh '''
                        if ! command -v sah &> /dev/null; then
                            curl -L https://releases.sah.dev/latest/sah-linux.tar.gz | tar xz
                            sudo mv sah /usr/local/bin/
                        fi
                    '''
                }
            }
        }
        
        stage('Test') {
            parallel {
                stage('Unit Tests') {
                    steps {
                        sh 'sah shell -t 900 --format json "cargo test --verbose" > test-results.json'
                        
                        script {
                            def result = readJSON file: 'test-results.json'
                            if (result.is_error) {
                                error("Tests failed: ${result.content}")
                            }
                        }
                    }
                    post {
                        always {
                            // Archive test results
                            archiveArtifacts artifacts: 'test-results.json', allowEmptyArchive: true
                        }
                    }
                }
                
                stage('Lint') {
                    steps {
                        sh '''
                            sah shell "cargo fmt -- --check"
                            sah shell "cargo clippy -- -D warnings"
                        '''
                    }
                }
                
                stage('Security Audit') {
                    steps {
                        sh 'sah shell -t 300 "cargo audit"'
                    }
                }
            }
        }
        
        stage('Build') {
            steps {
                sh '''
                    sah shell -t 1800 -e "RUSTFLAGS=-C target-cpu=native" \
                        "cargo build --release"
                '''
                
                // Archive build artifacts
                archiveArtifacts artifacts: 'target/release/myapp', fingerprint: true
            }
        }
        
        stage('Integration Tests') {
            steps {
                sh '''
                    # Start test services
                    sah shell "docker-compose -f docker-compose.test.yml up -d"
                    
                    # Wait for services to be ready
                    sah shell -t 120 "wait-for-it localhost:5432 --timeout=60"
                    
                    # Run integration tests
                    sah shell -t 900 "cargo test --test integration_tests"
                '''
            }
            post {
                always {
                    sh 'sah shell "docker-compose -f docker-compose.test.yml down"'
                }
            }
        }
        
        stage('Deploy') {
            when {
                branch 'main'
            }
            steps {
                script {
                    // Deploy to staging
                    sh '''
                        sah shell -t 300 "rsync -avz target/release/myapp \
                            deploy@staging.example.com:/opt/myapp/"
                        
                        sah shell -t 60 "ssh deploy@staging.example.com \
                            'systemctl restart myapp'"
                    '''
                    
                    // Health check
                    retry(3) {
                        sh 'sah shell -t 30 "curl -f http://staging.example.com/health"'
                    }
                }
            }
        }
    }
    
    post {
        always {
            // Clean up workspace
            sh 'sah shell "cargo clean"'
        }
        
        failure {
            // Collect failure artifacts
            sh '''
                sah shell --format json "cargo test 2>&1" > failure-log.json || true
                sah shell "docker logs \$(docker ps -q)" > container-logs.txt || true
            '''
            archiveArtifacts artifacts: 'failure-log.json,container-logs.txt', allowEmptyArchive: true
        }
    }
}
```

## Azure DevOps Integration

### Azure Pipelines

**Azure DevOps pipeline configuration**:
```yaml
# azure-pipelines.yml
trigger:
  branches:
    include:
    - main
    - develop

pool:
  vmImage: 'ubuntu-latest'

variables:
  RUST_LOG: 'info'
  CARGO_TERM_COLOR: 'always'

stages:
- stage: Test
  displayName: 'Test Stage'
  jobs:
  - job: UnitTests
    displayName: 'Unit Tests'
    steps:
    - task: Bash@3
      displayName: 'Install SAH'
      inputs:
        targetType: 'inline'
        script: |
          curl -L https://releases.sah.dev/latest/sah-linux.tar.gz | tar xz
          sudo mv sah /usr/local/bin/
    
    - task: Bash@3
      displayName: 'Run Tests'
      inputs:
        targetType: 'inline'
        script: |
          # Format check
          sah shell "cargo fmt -- --check"
          
          # Clippy lints
          sah shell "cargo clippy -- -D warnings"
          
          # Unit tests with JUnit output
          sah shell -t 900 -e "CARGO_TERM_COLOR=always" \
            "cargo test --verbose -- --format junit > test-results.xml"
    
    - task: PublishTestResults@2
      displayName: 'Publish Test Results'
      inputs:
        testResultsFormat: 'JUnit'
        testResultsFiles: 'test-results.xml'
        mergeTestResults: true

- stage: Build
  displayName: 'Build Stage'
  dependsOn: Test
  condition: succeeded()
  jobs:
  - job: BuildRelease
    displayName: 'Build Release'
    steps:
    - task: Bash@3
      displayName: 'Build Application'
      inputs:
        targetType: 'inline'
        script: |
          sah shell -t 1800 -e "RUSTFLAGS=-C target-cpu=native" \
            "cargo build --release"
          
          # Create distribution package
          sah shell "tar czf myapp-$(Build.BuildNumber).tar.gz \
            -C target/release myapp"
    
    - task: PublishBuildArtifacts@1
      displayName: 'Publish Artifacts'
      inputs:
        pathToPublish: 'myapp-$(Build.BuildNumber).tar.gz'
        artifactName: 'release-binary'

- stage: Deploy
  displayName: 'Deploy Stage'
  dependsOn: Build
  condition: and(succeeded(), eq(variables['Build.SourceBranch'], 'refs/heads/main'))
  jobs:
  - deployment: DeployToStaging
    displayName: 'Deploy to Staging'
    environment: 'staging'
    strategy:
      runOnce:
        deploy:
          steps:
          - download: current
            artifact: release-binary
          
          - task: Bash@3
            displayName: 'Deploy to Server'
            inputs:
              targetType: 'inline'
              script: |
                # Extract artifact
                tar xzf $(Pipeline.Workspace)/release-binary/myapp-$(Build.BuildNumber).tar.gz
                
                # Deploy via rsync
                sah shell -t 300 "rsync -avz myapp \
                  deploy@staging.example.com:/opt/myapp/"
                
                # Restart service
                sah shell -t 60 "ssh deploy@staging.example.com \
                  'systemctl restart myapp'"
                
                # Health check
                sah shell -t 30 "curl -f http://staging.example.com/health"
```

## Custom CI/CD Scripts

### Build Scripts

**Comprehensive build script**:
```bash
#!/bin/bash
# ci-build.sh

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_TYPE="${BUILD_TYPE:-release}"
TIMEOUT_BUILD="${TIMEOUT_BUILD:-1800}"
TIMEOUT_TEST="${TIMEOUT_TEST:-900}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
    exit 1
}

# Check if SAH is available
if ! command -v sah &> /dev/null; then
    error "SAH shell tool not found. Please install it first."
fi

log "Starting CI build process..."

# Clean previous build artifacts
log "Cleaning previous build artifacts..."
if ! sah shell -C "$PROJECT_ROOT" "cargo clean"; then
    warn "Failed to clean previous artifacts, continuing..."
fi

# Format check
log "Checking code formatting..."
if ! sah shell -C "$PROJECT_ROOT" "cargo fmt -- --check"; then
    error "Code formatting check failed. Run 'cargo fmt' to fix."
fi

# Linting
log "Running linting checks..."
if ! sah shell -C "$PROJECT_ROOT" "cargo clippy -- -D warnings"; then
    error "Linting failed. Fix clippy warnings before continuing."
fi

# Security audit
log "Running security audit..."
if ! sah shell -t 300 -C "$PROJECT_ROOT" "cargo audit"; then
    warn "Security audit found issues. Please review."
fi

# Unit tests
log "Running unit tests..."
if ! sah shell -t "$TIMEOUT_TEST" -C "$PROJECT_ROOT" \
    -e "RUST_LOG=info" \
    -e "RUST_BACKTRACE=1" \
    "cargo test --verbose"; then
    error "Unit tests failed."
fi

# Build
log "Building $BUILD_TYPE binary..."
if [ "$BUILD_TYPE" = "release" ]; then
    BUILD_CMD="cargo build --release"
    BINARY_PATH="target/release"
else
    BUILD_CMD="cargo build"
    BINARY_PATH="target/debug"
fi

if ! sah shell -t "$TIMEOUT_BUILD" -C "$PROJECT_ROOT" \
    -e "RUSTFLAGS=-C target-cpu=native" \
    "$BUILD_CMD"; then
    error "Build failed."
fi

# Integration tests (if they exist)
if [ -d "$PROJECT_ROOT/tests" ]; then
    log "Running integration tests..."
    if ! sah shell -t "$TIMEOUT_TEST" -C "$PROJECT_ROOT" \
        "cargo test --test integration_tests"; then
        error "Integration tests failed."
    fi
fi

# Performance tests (if configured)
if [ "${RUN_BENCHMARKS:-false}" = "true" ]; then
    log "Running performance benchmarks..."
    if ! sah shell -t 1800 -C "$PROJECT_ROOT" "cargo bench"; then
        warn "Benchmarks failed or not available."
    fi
fi

# Package binary
log "Creating distribution package..."
BINARY_NAME=$(grep '^name = ' Cargo.toml | sed 's/name = "//' | sed 's/"//')
VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "//' | sed 's/"//')
PACKAGE_NAME="${BINARY_NAME}-${VERSION}-$(date +%Y%m%d-%H%M%S)"

if ! sah shell -C "$PROJECT_ROOT" \
    "tar czf ${PACKAGE_NAME}.tar.gz -C $BINARY_PATH $BINARY_NAME"; then
    error "Failed to create distribution package."
fi

log "Build completed successfully!"
log "Package created: ${PACKAGE_NAME}.tar.gz"

# Upload artifacts (if configured)
if [ -n "${ARTIFACT_UPLOAD_CMD}" ]; then
    log "Uploading artifacts..."
    if ! sah shell -t 300 "$ARTIFACT_UPLOAD_CMD ${PACKAGE_NAME}.tar.gz"; then
        warn "Failed to upload artifacts."
    fi
fi

log "CI build process completed successfully!"
```

### Deployment Scripts

**Production deployment script**:
```bash
#!/bin/bash
# deploy.sh

set -e

# Configuration
ENVIRONMENT="${1:-staging}"
ARTIFACT_PATH="${2:-}"
DEPLOY_USER="${DEPLOY_USER:-deploy}"
DEPLOY_HOST="${DEPLOY_HOST:-}"
SERVICE_NAME="${SERVICE_NAME:-myapp}"
HEALTH_CHECK_URL="${HEALTH_CHECK_URL:-}"

# Validation
if [ -z "$DEPLOY_HOST" ]; then
    echo "Error: DEPLOY_HOST environment variable must be set"
    exit 1
fi

if [ -z "$ARTIFACT_PATH" ] || [ ! -f "$ARTIFACT_PATH" ]; then
    echo "Error: Valid artifact path must be provided"
    exit 1
fi

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1"
}

log "Starting deployment to $ENVIRONMENT..."

# Upload artifact
log "Uploading artifact to $DEPLOY_HOST..."
if ! sah shell -t 300 "scp $ARTIFACT_PATH $DEPLOY_USER@$DEPLOY_HOST:/tmp/"; then
    echo "Error: Failed to upload artifact"
    exit 1
fi

# Extract and install
ARTIFACT_NAME=$(basename "$ARTIFACT_PATH")
log "Installing application on remote host..."
if ! sah shell -t 120 "ssh $DEPLOY_USER@$DEPLOY_HOST '
    cd /opt/$SERVICE_NAME &&
    tar xzf /tmp/$ARTIFACT_NAME &&
    chmod +x $SERVICE_NAME &&
    sudo systemctl stop $SERVICE_NAME &&
    cp $SERVICE_NAME /usr/local/bin/ &&
    sudo systemctl start $SERVICE_NAME
'"; then
    echo "Error: Failed to install application"
    exit 1
fi

# Health check
if [ -n "$HEALTH_CHECK_URL" ]; then
    log "Performing health check..."
    sleep 5  # Give service time to start
    
    for i in {1..10}; do
        if sah shell -t 30 "curl -f $HEALTH_CHECK_URL"; then
            log "Health check passed!"
            break
        fi
        
        if [ $i -eq 10 ]; then
            echo "Error: Health check failed after 10 attempts"
            exit 1
        fi
        
        log "Health check attempt $i failed, retrying in 10 seconds..."
        sleep 10
    done
fi

log "Deployment to $ENVIRONMENT completed successfully!"
```

## Best Practices for CI/CD

### Error Handling and Retries

**Robust CI/CD with error handling**:
```bash
#!/bin/bash
# robust-ci.sh

# Retry function
retry() {
    local max_attempts="$1"
    local delay="$2" 
    local command="${@:3}"
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if eval "$command"; then
            return 0
        fi
        
        echo "Attempt $attempt failed. Retrying in ${delay}s..."
        sleep "$delay"
        ((attempt++))
    done
    
    echo "Command failed after $max_attempts attempts: $command"
    return 1
}

# Network-dependent operations with retry
retry 3 10 "sah shell -t 600 'npm install'"
retry 3 5 "sah shell -t 300 'cargo audit'"

# Critical operations with immediate failure
sah shell "cargo test" || {
    echo "Tests failed - collecting debug information..."
    sah shell "cargo test -- --nocapture" > test-debug.log 2>&1 || true
    exit 1
}
```

### Resource Management

**Efficient resource usage in CI**:
```bash
#!/bin/bash
# resource-efficient-ci.sh

# Detect available resources
CPU_CORES=$(nproc)
MEMORY_GB=$(( $(grep MemTotal /proc/meminfo | awk '{print $2}') / 1024 / 1024 ))

# Adjust parallelism based on resources
if [ $MEMORY_GB -gt 8 ]; then
    PARALLEL_JOBS=$CPU_CORES
else
    PARALLEL_JOBS=$((CPU_CORES / 2))
fi

log "Using $PARALLEL_JOBS parallel jobs on $CPU_CORES cores with ${MEMORY_GB}GB RAM"

# Build with appropriate parallelism
sah shell -t 1800 -e "CARGO_BUILD_JOBS=$PARALLEL_JOBS" "cargo build --release"

# Cleanup after each major step to preserve disk space
sah shell "cargo clean" || true
```

This CI/CD integration guide provides comprehensive examples for integrating the shell tool into various CI/CD platforms and custom automation scripts. Adapt the configurations and timeouts based on your specific project requirements and infrastructure capabilities.