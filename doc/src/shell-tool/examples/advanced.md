# Advanced Usage Examples

This guide demonstrates advanced usage patterns for the shell tool, including complex automation, performance optimization, security hardening, and integration with monitoring systems.

## Advanced Automation Patterns

### Conditional Command Execution

**Execute commands based on conditions**:
```bash
#!/bin/bash
# conditional-execution.sh

PROJECT_PATH="/path/to/project"

# Function to execute with condition checking
execute_with_condition() {
    local condition_cmd="$1"
    local execute_cmd="$2"
    local description="$3"
    
    echo "Checking condition: $description"
    
    if sah shell --quiet "$condition_cmd"; then
        echo "Condition met, executing: $execute_cmd"
        sah shell "$execute_cmd"
        return $?
    else
        echo "Condition not met, skipping: $execute_cmd"
        return 0
    fi
}

# Only build if source files have changed
execute_with_condition \
    "find $PROJECT_PATH/src -newer $PROJECT_PATH/target/release/myapp -type f | grep -q ." \
    "cargo build --release" \
    "Source files newer than binary"

# Only run tests if they exist
execute_with_condition \
    "test -d $PROJECT_PATH/tests" \
    "cargo test" \
    "Tests directory exists"

# Only deploy if all tests pass
execute_with_condition \
    "cargo test --quiet" \
    "rsync -avz target/release/myapp prod.example.com:/opt/myapp/" \
    "All tests pass"

# Only restart service if binary changed
execute_with_condition \
    "ssh prod.example.com 'test target/release/myapp -nt /opt/myapp/myapp'" \
    "ssh prod.example.com 'systemctl restart myapp'" \
    "Binary is newer than deployed version"
```

### Parallel Command Execution

**Execute multiple commands in parallel with synchronization**:
```bash
#!/bin/bash
# parallel-execution.sh

PROJECT_PATH="/path/to/project"
PIDS=()
RESULTS=()

# Function to run command in background and track PID
run_background() {
    local name="$1"
    local command="$2"
    local timeout="${3:-300}"
    
    echo "Starting background task: $name"
    
    (
        if sah shell -t "$timeout" "$command"; then
            echo "$name:SUCCESS:$?"
        else
            echo "$name:FAILURE:$?"
        fi
    ) > "/tmp/${name}.result" &
    
    PIDS+=($!)
    echo "Started $name with PID ${PIDS[-1]}"
}

# Start parallel tasks
run_background "format_check" "cargo fmt -- --check" 60
run_background "lint_check" "cargo clippy -- -D warnings" 300
run_background "unit_tests" "cargo test --lib" 600
run_background "integration_tests" "cargo test --test integration" 900
run_background "security_audit" "cargo audit" 300

# Wait for all background tasks to complete
echo "Waiting for all background tasks to complete..."
wait_start=$(date +%s)

for pid in "${PIDS[@]}"; do
    if wait "$pid"; then
        echo "Task with PID $pid completed successfully"
    else
        echo "Task with PID $pid failed"
    fi
done

wait_end=$(date +%s)
echo "All parallel tasks completed in $((wait_end - wait_start)) seconds"

# Collect and analyze results
echo "Task Results:"
for result_file in /tmp/*.result; do
    if [ -f "$result_file" ]; then
        cat "$result_file"
        rm -f "$result_file"
    fi
done

echo "Parallel execution completed"
```

### Multi-Environment Deployment

**Deploy to multiple environments with different configurations**:
```bash
#!/bin/bash
# multi-env-deployment.sh

PROJECT_PATH="/path/to/project"

# Environment configurations
declare -A ENVIRONMENTS=(
    ["dev"]="dev.example.com:/opt/myapp:8080"
    ["staging"]="staging.example.com:/opt/myapp:8081" 
    ["prod"]="prod.example.com:/opt/myapp:8082"
)

declare -A ENV_CONFIGS=(
    ["dev"]="DEBUG=true LOG_LEVEL=debug"
    ["staging"]="DEBUG=false LOG_LEVEL=info"
    ["prod"]="DEBUG=false LOG_LEVEL=warn OPTIMIZE=true"
)

# Function to deploy to specific environment
deploy_to_environment() {
    local env="$1"
    local env_config="${ENVIRONMENTS[$env]}"
    local env_vars="${ENV_CONFIGS[$env]}"
    
    if [ -z "$env_config" ]; then
        echo "Unknown environment: $env"
        return 1
    fi
    
    IFS=':' read -r host path port <<< "$env_config"
    
    echo "Deploying to $env environment ($host)"
    
    # Build with environment-specific configuration
    echo "Building for $env..."
    local build_cmd="cargo build --release"
    if [[ $env_vars == *"OPTIMIZE=true"* ]]; then
        build_cmd="$build_cmd --features optimize"
    fi
    
    if ! sah shell -t 1800 -C "$PROJECT_PATH" \
        -e "RUSTFLAGS=-C target-cpu=native" \
        "$build_cmd"; then
        echo "Build failed for $env"
        return 1
    fi
    
    # Create environment-specific configuration
    echo "Creating configuration for $env..."
    sah shell -C "$PROJECT_PATH" "cat > config-$env.toml << EOF
[server]
host = \"0.0.0.0\"
port = $port

[logging]
$(echo "$env_vars" | tr ' ' '\n' | grep LOG_LEVEL | sed 's/LOG_LEVEL=/level = \"/' | sed 's/$/\"/')

[features]
$(echo "$env_vars" | tr ' ' '\n' | grep DEBUG | sed 's/DEBUG=/debug = /' | tr '[:upper:]' '[:lower:]')
EOF"
    
    # Deploy binary and configuration
    echo "Deploying to $host..."
    if ! sah shell -t 300 \
        "rsync -avz $PROJECT_PATH/target/release/myapp $PROJECT_PATH/config-$env.toml $host:$path/"; then
        echo "Deployment failed for $env"
        return 1
    fi
    
    # Restart service
    echo "Restarting service on $env..."
    if ! sah shell -t 60 \
        "ssh ${host%:*} 'cd $path && systemctl restart myapp-$env'"; then
        echo "Service restart failed for $env"
        return 1
    fi
    
    # Health check
    echo "Performing health check for $env..."
    local health_url="http://${host%:*}:$port/health"
    
    for attempt in {1..10}; do
        if sah shell -t 30 "curl -f $health_url"; then
            echo "Health check passed for $env"
            break
        fi
        
        if [ $attempt -eq 10 ]; then
            echo "Health check failed for $env after 10 attempts"
            return 1
        fi
        
        echo "Health check attempt $attempt failed, retrying in 10 seconds..."
        sleep 10
    done
    
    echo "Successfully deployed to $env environment"
    return 0
}

# Deploy to environments in sequence with error handling
DEPLOY_ORDER=("dev" "staging" "prod")
FAILED_ENVS=()

for env in "${DEPLOY_ORDER[@]}"; do
    if deploy_to_environment "$env"; then
        echo "✓ $env deployment successful"
    else
        echo "✗ $env deployment failed"
        FAILED_ENVS+=("$env")
        
        # Stop deployment chain on production failure
        if [ "$env" = "prod" ]; then
            echo "Production deployment failed, stopping deployment chain"
            break
        fi
    fi
done

# Summary report
echo "Deployment Summary:"
for env in "${DEPLOY_ORDER[@]}"; do
    if [[ " ${FAILED_ENVS[@]} " =~ " $env " ]]; then
        echo "  $env: FAILED"
    else
        echo "  $env: SUCCESS"
    fi
done

if [ ${#FAILED_ENVS[@]} -eq 0 ]; then
    echo "All deployments completed successfully!"
    exit 0
else
    echo "Some deployments failed: ${FAILED_ENVS[*]}"
    exit 1
fi
```

## Performance Optimization

### Resource-Aware Command Execution

**Adapt command execution based on system resources**:
```bash
#!/bin/bash
# resource-aware-execution.sh

# Function to get system resources
get_system_resources() {
    local cpu_cores=$(nproc)
    local memory_gb=$(( $(grep MemTotal /proc/meminfo | awk '{print $2}') / 1024 / 1024 ))
    local load_avg=$(uptime | awk '{print $10}' | sed 's/,//')
    
    echo "CPU_CORES=$cpu_cores MEMORY_GB=$memory_gb LOAD_AVG=$load_avg"
}

# Function to calculate optimal parallel jobs
calculate_parallel_jobs() {
    local cpu_cores="$1"
    local memory_gb="$2"
    local load_avg="$3"
    
    # Base calculation on CPU cores
    local parallel_jobs=$cpu_cores
    
    # Reduce if memory is limited
    if [ "$memory_gb" -lt 4 ]; then
        parallel_jobs=$((parallel_jobs / 2))
    elif [ "$memory_gb" -lt 8 ]; then
        parallel_jobs=$((parallel_jobs * 3 / 4))
    fi
    
    # Reduce if system load is high
    if [ "$(echo "$load_avg > $cpu_cores" | bc)" -eq 1 ]; then
        parallel_jobs=$((parallel_jobs / 2))
    fi
    
    # Minimum of 1
    [ "$parallel_jobs" -lt 1 ] && parallel_jobs=1
    
    echo "$parallel_jobs"
}

# Function to calculate optimal timeout
calculate_timeout() {
    local base_timeout="$1"
    local cpu_cores="$2"
    local memory_gb="$3"
    
    local timeout_multiplier=1
    
    # Increase timeout on slower systems
    if [ "$cpu_cores" -lt 4 ]; then
        timeout_multiplier=2
    elif [ "$memory_gb" -lt 4 ]; then
        timeout_multiplier=2
    fi
    
    echo $((base_timeout * timeout_multiplier))
}

# Get current system resources
eval $(get_system_resources)
echo "System resources: $CPU_CORES cores, ${MEMORY_GB}GB RAM, load average: $LOAD_AVG"

# Calculate optimal settings
PARALLEL_JOBS=$(calculate_parallel_jobs "$CPU_CORES" "$MEMORY_GB" "$LOAD_AVG")
BUILD_TIMEOUT=$(calculate_timeout 1800 "$CPU_CORES" "$MEMORY_GB")
TEST_TIMEOUT=$(calculate_timeout 900 "$CPU_CORES" "$MEMORY_GB")

echo "Optimal settings: $PARALLEL_JOBS parallel jobs, ${BUILD_TIMEOUT}s build timeout, ${TEST_TIMEOUT}s test timeout"

# Execute with optimized settings
PROJECT_PATH="/path/to/project"

echo "Running tests with optimized settings..."
sah shell -t "$TEST_TIMEOUT" -C "$PROJECT_PATH" \
    -e "RUST_TEST_THREADS=$PARALLEL_JOBS" \
    "cargo test --verbose"

echo "Building with optimized settings..."
sah shell -t "$BUILD_TIMEOUT" -C "$PROJECT_PATH" \
    -e "CARGO_BUILD_JOBS=$PARALLEL_JOBS" \
    -e "RUSTFLAGS=-C target-cpu=native -C opt-level=3" \
    "cargo build --release"

echo "Performance-optimized build completed"
```

### Caching and Incremental Builds

**Implement intelligent caching for faster builds**:
```bash
#!/bin/bash
# cached-build-system.sh

PROJECT_PATH="/path/to/project"
CACHE_DIR="$HOME/.cache/build-cache"
BUILD_HASH_FILE="$CACHE_DIR/last-build-hash"

# Create cache directory
mkdir -p "$CACHE_DIR"

# Function to calculate source code hash
calculate_source_hash() {
    local project_path="$1"
    find "$project_path/src" "$project_path/Cargo.toml" "$project_path/Cargo.lock" \
        -type f -exec sha256sum {} \; 2>/dev/null | sort | sha256sum | awk '{print $1}'
}

# Function to check if build is needed
needs_build() {
    local current_hash="$1"
    
    if [ ! -f "$BUILD_HASH_FILE" ]; then
        echo "No previous build hash found"
        return 0
    fi
    
    local last_hash=$(cat "$BUILD_HASH_FILE")
    
    if [ "$current_hash" != "$last_hash" ]; then
        echo "Source code changed (hash: $current_hash != $last_hash)"
        return 0
    else
        echo "Source code unchanged (hash: $current_hash)"
        return 1
    fi
}

# Function to perform cached build
cached_build() {
    local project_path="$1"
    local build_type="${2:-release}"
    
    echo "Starting cached build process..."
    
    # Calculate current source hash
    local current_hash=$(calculate_source_hash "$project_path")
    echo "Current source hash: $current_hash"
    
    # Check if build is needed
    if needs_build "$current_hash"; then
        echo "Build required, starting compilation..."
        
        # Perform build with dependency caching
        local cargo_cache_dir="$CACHE_DIR/cargo"
        mkdir -p "$cargo_cache_dir"
        
        local build_start=$(date +%s)
        
        if sah shell -t 1800 -C "$project_path" \
            -e "CARGO_HOME=$cargo_cache_dir" \
            -e "SCCACHE_DIR=$CACHE_DIR/sccache" \
            -e "RUSTC_WRAPPER=sccache" \
            "cargo build --$build_type"; then
            
            local build_end=$(date +%s)
            local build_time=$((build_end - build_start))
            
            # Save successful build hash
            echo "$current_hash" > "$BUILD_HASH_FILE"
            echo "Build completed successfully in ${build_time}s"
            
            # Cache build artifacts
            echo "Caching build artifacts..."
            rsync -a "$project_path/target/" "$CACHE_DIR/target/" || true
            
            return 0
        else
            echo "Build failed"
            return 1
        fi
    else
        echo "Build not needed, using cached artifacts..."
        
        # Restore cached artifacts if needed
        if [ -d "$CACHE_DIR/target" ] && [ ! -f "$project_path/target/release/myapp" ]; then
            echo "Restoring cached build artifacts..."
            rsync -a "$CACHE_DIR/target/" "$project_path/target/"
        fi
        
        return 0
    fi
}

# Function to perform incremental tests
incremental_tests() {
    local project_path="$1"
    
    echo "Running incremental tests..."
    
    # Get list of changed files since last commit
    local changed_files
    changed_files=$(sah shell -C "$project_path" "git diff --name-only HEAD~1 HEAD" | grep -E '\.(rs|toml)$' || true)
    
    if [ -z "$changed_files" ]; then
        echo "No relevant files changed, running minimal test suite..."
        sah shell -t 300 -C "$project_path" "cargo test --lib --quiet"
    else
        echo "Files changed: $changed_files"
        echo "Running full test suite..."
        sah shell -t 900 -C "$project_path" \
            -e "RUST_LOG=info" \
            "cargo test --verbose"
    fi
}

# Main execution
echo "Starting intelligent build system..."

# Perform cached build
if cached_build "$PROJECT_PATH" "release"; then
    echo "Build phase completed successfully"
    
    # Run incremental tests
    if incremental_tests "$PROJECT_PATH"; then
        echo "Test phase completed successfully"
    else
        echo "Test phase failed"
        exit 1
    fi
else
    echo "Build phase failed"
    exit 1
fi

echo "Intelligent build system completed"
```

## Security Hardening

### Secure Multi-User Environment

**Execute commands with proper user isolation and security controls**:
```bash
#!/bin/bash
# secure-multi-user-execution.sh

# Configuration
ALLOWED_USERS=("developer1" "developer2" "ci-user")
ALLOWED_COMMANDS=("cargo build" "cargo test" "cargo check" "cargo fmt")
SECURE_WORKSPACE="/secure/workspace"
AUDIT_LOG="/var/log/secure-shell-audit.log"

# Function to audit log all activities
audit_log() {
    local user="$1"
    local command="$2"
    local result="$3"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    echo "[$timestamp] USER:$user COMMAND:$command RESULT:$result" >> "$AUDIT_LOG"
}

# Function to validate user
validate_user() {
    local user="$1"
    
    for allowed_user in "${ALLOWED_USERS[@]}"; do
        if [ "$user" = "$allowed_user" ]; then
            return 0
        fi
    done
    
    return 1
}

# Function to validate command
validate_command() {
    local command="$1"
    
    for allowed_command in "${ALLOWED_COMMANDS[@]}"; do
        if [[ "$command" == "$allowed_command"* ]]; then
            return 0
        fi
    done
    
    return 1
}

# Function to sanitize workspace path
sanitize_workspace() {
    local requested_path="$1"
    local user="$2"
    
    # Ensure path is within secure workspace
    local user_workspace="$SECURE_WORKSPACE/$user"
    local resolved_path=$(realpath "$user_workspace/$requested_path" 2>/dev/null)
    
    if [[ "$resolved_path" == "$user_workspace"* ]]; then
        echo "$resolved_path"
        return 0
    else
        echo "ERROR: Path outside user workspace"
        return 1
    fi
}

# Function to execute command securely
secure_execute() {
    local user="$1"
    local command="$2"
    local workspace="$3"
    local timeout="${4:-300}"
    
    # Validate inputs
    if ! validate_user "$user"; then
        audit_log "$user" "$command" "DENIED_USER"
        echo "Error: User not authorized"
        return 1
    fi
    
    if ! validate_command "$command"; then
        audit_log "$user" "$command" "DENIED_COMMAND"
        echo "Error: Command not allowed"
        return 1
    fi
    
    local safe_workspace
    if ! safe_workspace=$(sanitize_workspace "$workspace" "$user"); then
        audit_log "$user" "$command" "DENIED_WORKSPACE"
        echo "Error: Workspace path not allowed"
        return 1
    fi
    
    # Create user workspace if it doesn't exist
    mkdir -p "$safe_workspace"
    chown "$user:$user" "$safe_workspace"
    
    # Execute command with restricted environment
    local start_time=$(date +%s)
    
    if sah shell -t "$timeout" -C "$safe_workspace" \
        -e "USER=$user" \
        -e "HOME=$SECURE_WORKSPACE/$user" \
        -e "PATH=/usr/local/bin:/usr/bin:/bin" \
        -e "RUST_LOG=warn" \
        "$command"; then
        
        local end_time=$(date +%s)
        local execution_time=$((end_time - start_time))
        
        audit_log "$user" "$command" "SUCCESS:${execution_time}s"
        echo "Command executed successfully in ${execution_time}s"
        return 0
    else
        local end_time=$(date +%s)
        local execution_time=$((end_time - start_time))
        
        audit_log "$user" "$command" "FAILURE:${execution_time}s"
        echo "Command failed after ${execution_time}s"
        return 1
    fi
}

# Main execution interface
if [ $# -lt 3 ]; then
    echo "Usage: $0 <user> <command> <workspace> [timeout]"
    echo "Example: $0 developer1 'cargo test' project1 600"
    exit 1
fi

USER="$1"
COMMAND="$2"
WORKSPACE="$3"
TIMEOUT="${4:-300}"

echo "Executing secure command for user: $USER"
secure_execute "$USER" "$COMMAND" "$WORKSPACE" "$TIMEOUT"
```

### Compliance and Auditing

**Implement comprehensive auditing for compliance requirements**:
```bash
#!/bin/bash
# compliance-auditing.sh

# Compliance configuration
COMPLIANCE_LOG="/var/log/compliance-audit.log"
SENSITIVE_PATTERNS=("password" "secret" "token" "key" "credential")
REQUIRED_APPROVALS=("security-team" "compliance-officer")
COMPLIANCE_DB="/var/db/compliance-commands.db"

# Function to log compliance events
compliance_log() {
    local event_type="$1"
    local user="$2" 
    local command="$3"
    local additional_info="$4"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    local event_id=$(uuidgen)
    
    local log_entry="ID:$event_id TIMESTAMP:$timestamp TYPE:$event_type USER:$user COMMAND:$command INFO:$additional_info"
    echo "$log_entry" >> "$COMPLIANCE_LOG"
    
    # Store in compliance database
    sqlite3 "$COMPLIANCE_DB" "INSERT INTO compliance_events (id, timestamp, type, user, command, info) VALUES ('$event_id', '$timestamp', '$event_type', '$user', '$command', '$additional_info');" 2>/dev/null || true
}

# Function to check for sensitive data in commands
scan_sensitive_data() {
    local command="$1"
    
    for pattern in "${SENSITIVE_PATTERNS[@]}"; do
        if echo "$command" | grep -qi "$pattern"; then
            echo "SENSITIVE_DATA_DETECTED:$pattern"
            return 0
        fi
    done
    
    echo "CLEAN"
    return 1
}

# Function to check if command requires approval
requires_approval() {
    local command="$1"
    
    # Commands that modify production systems
    if echo "$command" | grep -qE "(systemctl|service|mount|umount|fdisk|mkfs)"; then
        echo "SYSTEM_MODIFICATION"
        return 0
    fi
    
    # Commands that access sensitive directories
    if echo "$command" | grep -qE "(/etc|/var/log|/root)"; then
        echo "SENSITIVE_ACCESS"
        return 0
    fi
    
    # Commands with privilege escalation
    if echo "$command" | grep -qE "(sudo|su|chmod|chown)"; then
        echo "PRIVILEGE_ESCALATION"
        return 0
    fi
    
    return 1
}

# Function to get command approval
get_approval() {
    local command="$1"
    local reason="$2"
    local user="$3"
    local approval_id=$(uuidgen)
    
    echo "Command requires approval: $reason"
    echo "Approval ID: $approval_id"
    
    # In a real implementation, this would integrate with approval workflow system
    echo "Please request approval from: ${REQUIRED_APPROVALS[*]}"
    echo "Command: $command"
    echo "User: $user"
    echo "Reason: $reason"
    
    # For demo purposes, simulate approval process
    read -p "Has approval been granted? (approval_id): " provided_approval_id
    
    if [ "$provided_approval_id" = "$approval_id" ]; then
        compliance_log "APPROVAL_GRANTED" "$user" "$command" "ApprovalID:$approval_id"
        return 0
    else
        compliance_log "APPROVAL_DENIED" "$user" "$command" "ApprovalID:$approval_id"
        return 1
    fi
}

# Function to execute with compliance controls
compliant_execute() {
    local user="$1"
    local command="$2"
    local workspace="$3"
    local timeout="${4:-300}"
    
    compliance_log "EXECUTION_REQUESTED" "$user" "$command" "Workspace:$workspace"
    
    # Scan for sensitive data
    local sensitivity_scan
    sensitivity_scan=$(scan_sensitive_data "$command")
    
    if [ "$sensitivity_scan" != "CLEAN" ]; then
        compliance_log "SENSITIVE_DATA_BLOCKED" "$user" "$command" "$sensitivity_scan"
        echo "Error: Command contains sensitive data patterns"
        return 1
    fi
    
    # Check if approval is required
    local approval_reason
    if approval_reason=$(requires_approval "$command"); then
        compliance_log "APPROVAL_REQUIRED" "$user" "$command" "$approval_reason"
        
        if ! get_approval "$command" "$approval_reason" "$user"; then
            compliance_log "EXECUTION_BLOCKED" "$user" "$command" "ApprovalDenied"
            echo "Error: Command execution not approved"
            return 1
        fi
    fi
    
    # Execute with full monitoring
    local start_time=$(date +%s)
    local temp_output=$(mktemp)
    local temp_error=$(mktemp)
    
    compliance_log "EXECUTION_STARTED" "$user" "$command" "StartTime:$start_time"
    
    if sah shell -t "$timeout" -C "$workspace" \
        --format json \
        "$command" > "$temp_output" 2> "$temp_error"; then
        
        local end_time=$(date +%s)
        local execution_time=$((end_time - start_time))
        local exit_code=0
        
        # Parse execution results
        local stdout=$(jq -r '.metadata.stdout' "$temp_output" 2>/dev/null || echo "")
        local stderr=$(jq -r '.metadata.stderr' "$temp_error" 2>/dev/null || echo "")
        
        compliance_log "EXECUTION_SUCCESS" "$user" "$command" "Duration:${execution_time}s ExitCode:$exit_code"
        
        # Store execution artifacts for compliance
        local artifact_dir="/var/compliance/artifacts/$(date +%Y%m%d)/$user"
        mkdir -p "$artifact_dir"
        
        local artifact_file="$artifact_dir/$(date +%H%M%S)-$(echo "$command" | md5sum | cut -d' ' -f1).log"
        {
            echo "COMPLIANCE ARTIFACT"
            echo "User: $user"
            echo "Command: $command"
            echo "Workspace: $workspace"
            echo "Start Time: $(date -d @$start_time)"
            echo "End Time: $(date -d @$end_time)"
            echo "Duration: ${execution_time}s"
            echo "Exit Code: $exit_code"
            echo "--- STDOUT ---"
            echo "$stdout"
            echo "--- STDERR ---"
            echo "$stderr"
            echo "--- END ARTIFACT ---"
        } > "$artifact_file"
        
        echo "Command executed successfully (Compliance ID: $(basename "$artifact_file"))"
        cat "$temp_output"
        
        cleanup_temps "$temp_output" "$temp_error"
        return 0
        
    else
        local end_time=$(date +%s)
        local execution_time=$((end_time - start_time))
        local exit_code=$?
        
        compliance_log "EXECUTION_FAILURE" "$user" "$command" "Duration:${execution_time}s ExitCode:$exit_code"
        
        echo "Command failed with exit code $exit_code"
        cat "$temp_error" >&2
        
        cleanup_temps "$temp_output" "$temp_error"
        return $exit_code
    fi
}

# Function to clean up temporary files
cleanup_temps() {
    local temp_output="$1"
    local temp_error="$2"
    
    [ -f "$temp_output" ] && rm -f "$temp_output"
    [ -f "$temp_error" ] && rm -f "$temp_error"
}

# Initialize compliance database
initialize_compliance_db() {
    if [ ! -f "$COMPLIANCE_DB" ]; then
        sqlite3 "$COMPLIANCE_DB" "
            CREATE TABLE compliance_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT,
                type TEXT,
                user TEXT,
                command TEXT,
                info TEXT
            );
            CREATE INDEX idx_timestamp ON compliance_events(timestamp);
            CREATE INDEX idx_user ON compliance_events(user);
            CREATE INDEX idx_type ON compliance_events(type);
        " 2>/dev/null || echo "Warning: Could not create compliance database"
    fi
}

# Function to generate compliance reports
generate_compliance_report() {
    local start_date="$1"
    local end_date="$2"
    local report_file="/var/compliance/reports/report-$(date +%Y%m%d-%H%M%S).txt"
    
    mkdir -p "$(dirname "$report_file")"
    
    {
        echo "COMPLIANCE AUDIT REPORT"
        echo "Period: $start_date to $end_date"
        echo "Generated: $(date)"
        echo ""
        
        echo "SUMMARY STATISTICS:"
        sqlite3 "$COMPLIANCE_DB" "
            SELECT 
                type,
                COUNT(*) as count
            FROM compliance_events 
            WHERE timestamp BETWEEN '$start_date' AND '$end_date'
            GROUP BY type
            ORDER BY count DESC;
        " 2>/dev/null || echo "Database query failed"
        
        echo ""
        echo "USER ACTIVITY:"
        sqlite3 "$COMPLIANCE_DB" "
            SELECT 
                user,
                COUNT(*) as command_count,
                COUNT(CASE WHEN type = 'EXECUTION_SUCCESS' THEN 1 END) as successful,
                COUNT(CASE WHEN type = 'EXECUTION_FAILURE' THEN 1 END) as failed
            FROM compliance_events 
            WHERE timestamp BETWEEN '$start_date' AND '$end_date'
            AND type IN ('EXECUTION_SUCCESS', 'EXECUTION_FAILURE')
            GROUP BY user
            ORDER BY command_count DESC;
        " 2>/dev/null || echo "Database query failed"
        
        echo ""
        echo "SECURITY EVENTS:"
        sqlite3 "$COMPLIANCE_DB" "
            SELECT 
                timestamp,
                user,
                command,
                info
            FROM compliance_events 
            WHERE timestamp BETWEEN '$start_date' AND '$end_date'
            AND type IN ('SENSITIVE_DATA_BLOCKED', 'APPROVAL_DENIED', 'EXECUTION_BLOCKED')
            ORDER BY timestamp DESC;
        " 2>/dev/null || echo "Database query failed"
        
    } > "$report_file"
    
    echo "Compliance report generated: $report_file"
}

# Main execution
case "$1" in
    "execute")
        if [ $# -lt 4 ]; then
            echo "Usage: $0 execute <user> <command> <workspace> [timeout]"
            exit 1
        fi
        
        initialize_compliance_db
        compliant_execute "$2" "$3" "$4" "$5"
        ;;
    
    "report")
        if [ $# -lt 3 ]; then
            echo "Usage: $0 report <start_date> <end_date>"
            echo "Example: $0 report '2024-01-01' '2024-01-31'"
            exit 1
        fi
        
        generate_compliance_report "$2" "$3"
        ;;
    
    *)
        echo "Usage: $0 {execute|report}"
        echo "  execute <user> <command> <workspace> [timeout] - Execute command with compliance controls"
        echo "  report <start_date> <end_date> - Generate compliance report"
        exit 1
        ;;
esac
```

## Monitoring and Observability

### Comprehensive Metrics Collection

**Collect detailed metrics about shell command execution**:
```bash
#!/bin/bash
# metrics-collection.sh

METRICS_DIR="/var/metrics/shell-tool"
PROMETHEUS_FILE="$METRICS_DIR/shell_tool_metrics.prom"

# Initialize metrics directory
mkdir -p "$METRICS_DIR"

# Function to record metric
record_metric() {
    local metric_name="$1"
    local metric_value="$2"
    local labels="$3"
    local help_text="$4"
    local metric_type="${5:-counter}"
    
    local timestamp=$(date +%s)
    
    # Prometheus format
    {
        echo "# HELP $metric_name $help_text"
        echo "# TYPE $metric_name $metric_type"
        echo "$metric_name{$labels} $metric_value $timestamp"
    } >> "$PROMETHEUS_FILE"
    
    # JSON format for other collectors
    jq -n \
        --arg name "$metric_name" \
        --arg value "$metric_value" \
        --arg labels "$labels" \
        --arg help "$help_text" \
        --arg type "$metric_type" \
        --arg timestamp "$timestamp" \
        '{
            name: $name,
            value: ($value | tonumber),
            labels: $labels,
            help: $help,
            type: $type,
            timestamp: ($timestamp | tonumber)
        }' >> "$METRICS_DIR/metrics-$(date +%Y%m%d).jsonl"
}

# Function to execute command with metrics collection
execute_with_metrics() {
    local command="$1"
    local working_directory="$2"
    local timeout="${3:-300}"
    local user="${4:-$(whoami)}"
    local environment="$5"
    
    local start_time=$(date +%s)
    local start_time_ns=$(date +%s%N)
    
    # Record execution start
    record_metric "shell_command_started_total" "1" \
        "user=\"$user\",command_hash=\"$(echo "$command" | md5sum | cut -d' ' -f1)\"" \
        "Total number of shell commands started"
    
    # Execute command with full output capture
    local temp_output=$(mktemp)
    local temp_error=$(mktemp)
    local exit_code=0
    
    if sah shell -t "$timeout" -C "$working_directory" \
        --format json --show-metadata \
        ${environment:+-e "$environment"} \
        "$command" > "$temp_output" 2> "$temp_error"; then
        exit_code=0
    else
        exit_code=$?
    fi
    
    local end_time=$(date +%s)
    local end_time_ns=$(date +%s%N)
    local duration_seconds=$((end_time - start_time))
    local duration_ms=$(( (end_time_ns - start_time_ns) / 1000000 ))
    
    # Parse command output for additional metrics
    local stdout_length=0
    local stderr_length=0
    local actual_exit_code=0
    
    if [ -f "$temp_output" ]; then
        local metadata=$(jq -r '.metadata // {}' "$temp_output" 2>/dev/null || echo '{}')
        stdout_length=$(echo "$metadata" | jq -r '.stdout // "" | length' 2>/dev/null || echo 0)
        stderr_length=$(echo "$metadata" | jq -r '.stderr // "" | length' 2>/dev/null || echo 0)
        actual_exit_code=$(echo "$metadata" | jq -r '.exit_code // 0' 2>/dev/null || echo 0)
    fi
    
    local labels="user=\"$user\",command_hash=\"$(echo "$command" | md5sum | cut -d' ' -f1)\",exit_code=\"$actual_exit_code\""
    
    # Record comprehensive metrics
    record_metric "shell_command_duration_seconds" "$duration_seconds" "$labels" \
        "Duration of shell command execution in seconds" "histogram"
    
    record_metric "shell_command_duration_milliseconds" "$duration_ms" "$labels" \
        "Duration of shell command execution in milliseconds" "histogram"
    
    record_metric "shell_command_stdout_bytes" "$stdout_length" "$labels" \
        "Size of stdout output in bytes" "histogram"
    
    record_metric "shell_command_stderr_bytes" "$stderr_length" "$labels" \
        "Size of stderr output in bytes" "histogram"
    
    if [ $exit_code -eq 0 ]; then
        record_metric "shell_command_success_total" "1" "$labels" \
            "Total number of successful shell commands"
    else
        record_metric "shell_command_failure_total" "1" "$labels" \
            "Total number of failed shell commands"
    fi
    
    # Timeout detection
    if [ $duration_seconds -ge $timeout ]; then
        record_metric "shell_command_timeout_total" "1" "$labels" \
            "Total number of shell commands that timed out"
    fi
    
    # Resource usage metrics (if available)
    local cpu_percent=$(ps -o %cpu -p $$ | tail -1 | tr -d ' ' | cut -d. -f1 2>/dev/null || echo 0)
    local memory_kb=$(ps -o rss -p $$ | tail -1 | tr -d ' ' 2>/dev/null || echo 0)
    
    record_metric "shell_command_cpu_percent" "$cpu_percent" "$labels" \
        "CPU usage percentage during command execution" "gauge"
    
    record_metric "shell_command_memory_kb" "$memory_kb" "$labels" \
        "Memory usage in KB during command execution" "gauge"
    
    # Clean up
    rm -f "$temp_output" "$temp_error"
    
    # Display results
    echo "Command executed in ${duration_seconds}s (exit code: $actual_exit_code)"
    return $exit_code
}

# Function to generate system metrics
collect_system_metrics() {
    local cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | sed 's/%us,//')
    local memory_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
    local disk_usage=$(df / | tail -1 | awk '{print $5}' | sed 's/%//')
    local load_avg=$(uptime | awk '{print $10}' | sed 's/,//')
    
    local system_labels="hostname=\"$(hostname)\""
    
    record_metric "system_cpu_usage_percent" "$cpu_usage" "$system_labels" \
        "System CPU usage percentage" "gauge"
    
    record_metric "system_memory_usage_percent" "$memory_usage" "$system_labels" \
        "System memory usage percentage" "gauge"
    
    record_metric "system_disk_usage_percent" "$disk_usage" "$system_labels" \
        "System disk usage percentage" "gauge"
    
    record_metric "system_load_average" "$load_avg" "$system_labels" \
        "System load average" "gauge"
}

# Function to rotate metrics files
rotate_metrics() {
    local retention_days="${1:-30}"
    
    # Compress old metrics files
    find "$METRICS_DIR" -name "metrics-*.jsonl" -mtime +1 -exec gzip {} \;
    
    # Remove old compressed files
    find "$METRICS_DIR" -name "metrics-*.jsonl.gz" -mtime +$retention_days -delete
    
    # Rotate Prometheus file if it gets too large
    if [ -f "$PROMETHEUS_FILE" ] && [ $(wc -l < "$PROMETHEUS_FILE") -gt 10000 ]; then
        local backup_file="$PROMETHEUS_FILE.$(date +%Y%m%d-%H%M%S)"
        mv "$PROMETHEUS_FILE" "$backup_file"
        gzip "$backup_file"
        touch "$PROMETHEUS_FILE"
    fi
}

# Main execution
case "$1" in
    "execute")
        shift
        execute_with_metrics "$@"
        ;;
    
    "system")
        collect_system_metrics
        ;;
    
    "rotate")
        rotate_metrics "$2"
        ;;
    
    *)
        echo "Usage: $0 {execute|system|rotate}"
        echo "  execute <command> [working_dir] [timeout] [user] [env] - Execute command with metrics"
        echo "  system - Collect system metrics"
        echo "  rotate [retention_days] - Rotate metrics files"
        exit 1
        ;;
esac
```

This advanced usage guide demonstrates sophisticated patterns for automation, performance optimization, security hardening, and monitoring. These examples can be adapted and extended for specific enterprise requirements and use cases.