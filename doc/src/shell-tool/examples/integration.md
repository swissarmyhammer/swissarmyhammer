# Integration Examples

This guide demonstrates how to integrate the shell tool with various systems, including MCP clients, workflow systems, and third-party tools.

## MCP Protocol Integration

### JavaScript MCP Client

**Basic MCP client integration**:
```javascript
// mcp-client.js
import { MCPClient } from '@modelcontextprotocol/client';

class ShellToolClient {
    constructor(serverUrl) {
        this.client = new MCPClient();
        this.serverUrl = serverUrl;
    }
    
    async connect() {
        await this.client.connect(this.serverUrl);
    }
    
    async executeCommand(command, options = {}) {
        const params = {
            command,
            ...options
        };
        
        try {
            const result = await this.client.callTool('shell_execute', params);
            return this.processResult(result);
        } catch (error) {
            console.error('Shell command failed:', error);
            throw error;
        }
    }
    
    processResult(result) {
        if (result.is_error) {
            throw new Error(`Command failed: ${result.content[0].text}`);
        }
        
        return {
            success: true,
            output: result.metadata.stdout,
            error: result.metadata.stderr,
            exitCode: result.metadata.exit_code,
            executionTime: result.metadata.execution_time_ms
        };
    }
    
    async buildProject(projectPath, buildType = 'release') {
        const buildCommand = buildType === 'release' 
            ? 'cargo build --release' 
            : 'cargo build';
            
        return await this.executeCommand(buildCommand, {
            working_directory: projectPath,
            timeout: 1800,
            environment: {
                RUST_LOG: 'info',
                CARGO_TERM_COLOR: 'always'
            }
        });
    }
    
    async runTests(projectPath, testType = 'unit') {
        const testCommand = testType === 'integration' 
            ? 'cargo test --test integration_tests'
            : 'cargo test';
            
        return await this.executeCommand(testCommand, {
            working_directory: projectPath,
            timeout: 900,
            environment: {
                RUST_LOG: 'debug',
                RUST_BACKTRACE: '1'
            }
        });
    }
}

// Usage example
async function example() {
    const client = new ShellToolClient('http://localhost:3000/mcp');
    await client.connect();
    
    try {
        // Build project
        console.log('Building project...');
        const buildResult = await client.buildProject('/path/to/project');
        console.log(`Build completed in ${buildResult.executionTime}ms`);
        
        // Run tests
        console.log('Running tests...');
        const testResult = await client.runTests('/path/to/project');
        console.log('Tests passed!');
        
    } catch (error) {
        console.error('Operation failed:', error.message);
    }
}
```

### Python MCP Client

**Python MCP integration example**:
```python
# mcp_shell_client.py
import asyncio
import json
from typing import Dict, Optional, Any
import aiohttp

class ShellToolClient:
    def __init__(self, server_url: str):
        self.server_url = server_url
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
    
    async def execute_command(
        self, 
        command: str,
        working_directory: Optional[str] = None,
        timeout: int = 300,
        environment: Optional[Dict[str, str]] = None
    ) -> Dict[str, Any]:
        """Execute a shell command via MCP."""
        
        params = {"command": command}
        if working_directory:
            params["working_directory"] = working_directory
        if timeout:
            params["timeout"] = timeout
        if environment:
            params["environment"] = environment
        
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "shell_execute",
                "arguments": params
            }
        }
        
        async with self.session.post(
            f"{self.server_url}/tools/call",
            json=payload,
            headers={"Content-Type": "application/json"}
        ) as response:
            result = await response.json()
            
            if "error" in result:
                raise Exception(f"MCP Error: {result['error']}")
            
            tool_result = result["result"]
            
            if tool_result.get("is_error", False):
                raise Exception(f"Command failed: {tool_result['content'][0]['text']}")
            
            return {
                "stdout": tool_result["metadata"]["stdout"],
                "stderr": tool_result["metadata"]["stderr"],
                "exit_code": tool_result["metadata"]["exit_code"],
                "execution_time_ms": tool_result["metadata"]["execution_time_ms"]
            }
    
    async def run_build_pipeline(self, project_path: str) -> Dict[str, Any]:
        """Run a complete build pipeline."""
        
        results = {}
        
        # Format check
        print("Checking code formatting...")
        results["format"] = await self.execute_command(
            "cargo fmt -- --check",
            working_directory=project_path
        )
        
        # Linting
        print("Running lints...")
        results["lint"] = await self.execute_command(
            "cargo clippy -- -D warnings",
            working_directory=project_path
        )
        
        # Tests
        print("Running tests...")
        results["test"] = await self.execute_command(
            "cargo test",
            working_directory=project_path,
            timeout=900,
            environment={"RUST_LOG": "debug"}
        )
        
        # Build
        print("Building release...")
        results["build"] = await self.execute_command(
            "cargo build --release",
            working_directory=project_path,
            timeout=1800,
            environment={"RUSTFLAGS": "-C target-cpu=native"}
        )
        
        return results

# Usage example
async def main():
    async with ShellToolClient("http://localhost:3000") as client:
        try:
            results = await client.run_build_pipeline("/path/to/rust/project")
            print("Build pipeline completed successfully!")
            
            for step, result in results.items():
                print(f"{step}: completed in {result['execution_time_ms']}ms")
                
        except Exception as e:
            print(f"Build pipeline failed: {e}")

if __name__ == "__main__":
    asyncio.run(main())
```

### Go MCP Client

**Go MCP integration example**:
```go
// mcp_shell_client.go
package main

import (
    "bytes"
    "context"
    "encoding/json"
    "fmt"
    "net/http"
    "time"
)

type ShellToolClient struct {
    serverURL  string
    httpClient *http.Client
}

type ShellExecuteParams struct {
    Command          string            `json:"command"`
    WorkingDirectory string            `json:"working_directory,omitempty"`
    Timeout          int               `json:"timeout,omitempty"`
    Environment      map[string]string `json:"environment,omitempty"`
}

type MCPRequest struct {
    JSONRPC string      `json:"jsonrpc"`
    ID      int         `json:"id"`
    Method  string      `json:"method"`
    Params  interface{} `json:"params"`
}

type ToolCallParams struct {
    Name      string      `json:"name"`
    Arguments interface{} `json:"arguments"`
}

type MCPResponse struct {
    ID     int                    `json:"id"`
    Result map[string]interface{} `json:"result,omitempty"`
    Error  map[string]interface{} `json:"error,omitempty"`
}

type ShellResult struct {
    Stdout          string `json:"stdout"`
    Stderr          string `json:"stderr"`
    ExitCode        int    `json:"exit_code"`
    ExecutionTimeMS int    `json:"execution_time_ms"`
}

func NewShellToolClient(serverURL string) *ShellToolClient {
    return &ShellToolClient{
        serverURL: serverURL,
        httpClient: &http.Client{
            Timeout: 30 * time.Second,
        },
    }
}

func (c *ShellToolClient) ExecuteCommand(ctx context.Context, params ShellExecuteParams) (*ShellResult, error) {
    request := MCPRequest{
        JSONRPC: "2.0",
        ID:      1,
        Method:  "tools/call",
        Params: ToolCallParams{
            Name:      "shell_execute",
            Arguments: params,
        },
    }
    
    jsonData, err := json.Marshal(request)
    if err != nil {
        return nil, fmt.Errorf("failed to marshal request: %w", err)
    }
    
    req, err := http.NewRequestWithContext(
        ctx, 
        "POST", 
        c.serverURL+"/tools/call",
        bytes.NewBuffer(jsonData),
    )
    if err != nil {
        return nil, fmt.Errorf("failed to create request: %w", err)
    }
    
    req.Header.Set("Content-Type", "application/json")
    
    resp, err := c.httpClient.Do(req)
    if err != nil {
        return nil, fmt.Errorf("request failed: %w", err)
    }
    defer resp.Body.Close()
    
    var mcpResp MCPResponse
    if err := json.NewDecoder(resp.Body).Decode(&mcpResp); err != nil {
        return nil, fmt.Errorf("failed to decode response: %w", err)
    }
    
    if mcpResp.Error != nil {
        return nil, fmt.Errorf("MCP error: %v", mcpResp.Error)
    }
    
    // Extract metadata from response
    metadata, ok := mcpResp.Result["metadata"].(map[string]interface{})
    if !ok {
        return nil, fmt.Errorf("invalid response format")
    }
    
    result := &ShellResult{
        Stdout:          metadata["stdout"].(string),
        Stderr:          metadata["stderr"].(string),
        ExitCode:        int(metadata["exit_code"].(float64)),
        ExecutionTimeMS: int(metadata["execution_time_ms"].(float64)),
    }
    
    return result, nil
}

func (c *ShellToolClient) BuildAndTest(ctx context.Context, projectPath string) error {
    // Format check
    fmt.Println("Checking formatting...")
    _, err := c.ExecuteCommand(ctx, ShellExecuteParams{
        Command:          "cargo fmt -- --check",
        WorkingDirectory: projectPath,
    })
    if err != nil {
        return fmt.Errorf("format check failed: %w", err)
    }
    
    // Linting
    fmt.Println("Running lints...")
    _, err = c.ExecuteCommand(ctx, ShellExecuteParams{
        Command:          "cargo clippy -- -D warnings",
        WorkingDirectory: projectPath,
    })
    if err != nil {
        return fmt.Errorf("linting failed: %w", err)
    }
    
    // Tests
    fmt.Println("Running tests...")
    testResult, err := c.ExecuteCommand(ctx, ShellExecuteParams{
        Command:          "cargo test",
        WorkingDirectory: projectPath,
        Timeout:          900,
        Environment: map[string]string{
            "RUST_LOG": "debug",
        },
    })
    if err != nil {
        return fmt.Errorf("tests failed: %w", err)
    }
    
    fmt.Printf("Tests completed in %dms\n", testResult.ExecutionTimeMS)
    
    // Build
    fmt.Println("Building release...")
    buildResult, err := c.ExecuteCommand(ctx, ShellExecuteParams{
        Command:          "cargo build --release",
        WorkingDirectory: projectPath,
        Timeout:          1800,
        Environment: map[string]string{
            "RUSTFLAGS": "-C target-cpu=native",
        },
    })
    if err != nil {
        return fmt.Errorf("build failed: %w", err)
    }
    
    fmt.Printf("Build completed in %dms\n", buildResult.ExecutionTimeMS)
    return nil
}

func main() {
    client := NewShellToolClient("http://localhost:3000")
    ctx := context.Background()
    
    if err := client.BuildAndTest(ctx, "/path/to/rust/project"); err != nil {
        fmt.Printf("Build and test failed: %v\n", err)
        return
    }
    
    fmt.Println("Build and test completed successfully!")
}
```

## Workflow System Integration

### SwissArmyHammer Workflow Integration

**Workflow definition with shell commands**:
```markdown
# build-and-deploy.md
name: "build_and_deploy"
description: "Complete build and deployment workflow"

states:
  - name: "setup"
    description: "Setup build environment" 
    actions:
      - type: "shell"
        command: "git clean -fd"
        working_directory: "/project"
        timeout: 60
        
      - type: "shell"
        command: "git pull origin main"
        working_directory: "/project" 
        timeout: 120

  - name: "test"
    description: "Run tests and quality checks"
    actions:
      - type: "shell"
        command: "cargo fmt -- --check"
        working_directory: "/project"
        timeout: 60
        
      - type: "shell"
        command: "cargo clippy -- -D warnings"  
        working_directory: "/project"
        timeout: 300
        
      - type: "shell"
        command: "cargo test"
        working_directory: "/project"
        timeout: 900
        environment:
          RUST_LOG: "debug"
          RUST_BACKTRACE: "1"

  - name: "build"
    description: "Build release binary"
    actions:
      - type: "shell"
        command: "cargo build --release"
        working_directory: "/project"
        timeout: 1800
        environment:
          RUSTFLAGS: "-C target-cpu=native"
          CARGO_TERM_COLOR: "always"

  - name: "package"
    description: "Create deployment package"
    actions:
      - type: "shell"
        command: "tar czf myapp-$(date +%Y%m%d-%H%M%S).tar.gz -C target/release myapp"
        working_directory: "/project"
        timeout: 300

  - name: "deploy"
    description: "Deploy to production"
    actions:
      - type: "shell" 
        command: "rsync -avz myapp-*.tar.gz deploy@prod.example.com:/opt/deploy/"
        working_directory: "/project"
        timeout: 600
        
      - type: "shell"
        command: "ssh deploy@prod.example.com 'cd /opt/deploy && ./deploy.sh'"
        timeout: 900

  - name: "verify"
    description: "Verify deployment"
    actions:
      - type: "shell"
        command: "curl -f http://prod.example.com/health"
        timeout: 30
        max_attempts: 5
        retry_delay: 10

transitions:
  - from: "setup"
    to: "test"
    
  - from: "test" 
    to: "build"
    
  - from: "build"
    to: "package"
    
  - from: "package"
    to: "deploy"
    
  - from: "deploy"
    to: "verify"
```

### Complex Workflow with Error Handling

**Advanced workflow with conditional logic**:
```markdown
# advanced-ci.md  
name: "advanced_ci"
description: "Advanced CI workflow with error handling and rollback"

states:
  - name: "pre_checks"
    description: "Pre-flight checks"
    actions:
      - type: "shell"
        command: "git status --porcelain | wc -l"
        working_directory: "/project"
        capture_output: true
        
      # Conditional check for uncommitted changes
      - type: "conditional"
        condition: "last_output.trim() != '0'"
        actions:
          - type: "log"
            message: "Warning: Uncommitted changes detected"

  - name: "backup"
    description: "Backup current production"
    actions:
      - type: "shell"
        command: "ssh prod.example.com 'systemctl stop myapp'"
        timeout: 60
        
      - type: "shell"
        command: "ssh prod.example.com 'cp /opt/myapp/myapp /opt/myapp/myapp.backup'"
        timeout: 120

  - name: "build_and_test"
    description: "Build and test with parallel execution"
    actions:
      - type: "parallel"
        actions:
          - type: "shell"
            name: "unit_tests"
            command: "cargo test --lib"
            working_directory: "/project"
            timeout: 600
            
          - type: "shell" 
            name: "integration_tests"
            command: "cargo test --test integration"
            working_directory: "/project" 
            timeout: 900
            
          - type: "shell"
            name: "security_audit"
            command: "cargo audit"
            working_directory: "/project"
            timeout: 300
            allow_failure: true

  - name: "deploy_staging"
    description: "Deploy to staging for final testing"
    actions:
      - type: "shell"
        command: "cargo build --release"
        working_directory: "/project"
        timeout: 1800
        
      - type: "shell"
        command: "scp target/release/myapp staging.example.com:/opt/myapp/"
        working_directory: "/project"
        timeout: 300
        
      - type: "shell"
        command: "ssh staging.example.com 'systemctl restart myapp'"
        timeout: 60
        
      # Health check with retry
      - type: "shell"
        command: "curl -f http://staging.example.com/health"
        timeout: 30
        max_attempts: 10
        retry_delay: 5

  - name: "production_deploy"
    description: "Deploy to production"
    actions:
      - type: "shell"
        command: "scp target/release/myapp prod.example.com:/opt/myapp/"
        working_directory: "/project"
        timeout: 300
        
      - type: "shell"
        command: "ssh prod.example.com 'systemctl start myapp'"  
        timeout: 60

  - name: "verify_production"
    description: "Verify production deployment"
    actions:
      - type: "shell"
        command: "curl -f http://prod.example.com/health"
        timeout: 30
        max_attempts: 5
        retry_delay: 10

  - name: "rollback"
    description: "Rollback on failure" 
    actions:
      - type: "shell"
        command: "ssh prod.example.com 'systemctl stop myapp'"
        timeout: 60
        
      - type: "shell" 
        command: "ssh prod.example.com 'cp /opt/myapp/myapp.backup /opt/myapp/myapp'"
        timeout: 120
        
      - type: "shell"
        command: "ssh prod.example.com 'systemctl start myapp'"
        timeout: 60

transitions:
  - from: "pre_checks"
    to: "backup"
    
  - from: "backup"
    to: "build_and_test"
    
  - from: "build_and_test"
    to: "deploy_staging"
    on_success: true
    
  - from: "build_and_test" 
    to: "rollback"
    on_failure: true
    
  - from: "deploy_staging"
    to: "production_deploy"
    on_success: true
    
  - from: "deploy_staging"
    to: "rollback" 
    on_failure: true
    
  - from: "production_deploy"
    to: "verify_production"
    
  - from: "verify_production"
    to: "complete"
    on_success: true
    
  - from: "verify_production"
    to: "rollback"
    on_failure: true

error_handling:
  default_action: "rollback"
  max_retries: 3
  notification:
    on_failure: "admin@example.com"
    on_success: "team@example.com"
```

## Third-Party Tool Integration

### Jenkins Integration via REST API

**Jenkins job that uses shell tool**:
```groovy
// jenkins-shell-integration.groovy
pipeline {
    agent any
    
    environment {
        SAH_SERVER_URL = 'http://localhost:3000'
    }
    
    stages {
        stage('Execute via Shell Tool') {
            steps {
                script {
                    // Define shell command parameters
                    def shellParams = [
                        command: 'cargo test --verbose',
                        working_directory: '/project',
                        timeout: 900,
                        environment: [
                            RUST_LOG: 'debug',
                            CI: 'true'
                        ]
                    ]
                    
                    // Call shell tool via HTTP
                    def response = httpRequest(
                        httpMode: 'POST',
                        url: "${SAH_SERVER_URL}/tools/call",
                        contentType: 'APPLICATION_JSON',
                        requestBody: groovy.json.JsonOutput.toJson([
                            jsonrpc: '2.0',
                            id: 1,
                            method: 'tools/call',
                            params: [
                                name: 'shell_execute',
                                arguments: shellParams
                            ]
                        ])
                    )
                    
                    def result = readJSON text: response.content
                    
                    if (result.error) {
                        error "Shell command failed: ${result.error}"
                    }
                    
                    if (result.result.is_error) {
                        error "Command failed: ${result.result.content[0].text}"
                    }
                    
                    echo "Command completed successfully in ${result.result.metadata.execution_time_ms}ms"
                    echo "Output: ${result.result.metadata.stdout}"
                }
            }
        }
    }
}
```

### Ansible Integration

**Ansible playbook using shell tool**:
```yaml
# ansible-shell-integration.yml
---
- name: Build and Deploy with Shell Tool
  hosts: localhost
  vars:
    sah_server_url: "http://localhost:3000"
    project_path: "/path/to/project"
    
  tasks:
    - name: Run tests via shell tool
      uri:
        url: "{{ sah_server_url }}/tools/call"
        method: POST
        body_format: json
        body:
          jsonrpc: "2.0"
          id: 1
          method: "tools/call"
          params:
            name: "shell_execute"
            arguments:
              command: "cargo test"
              working_directory: "{{ project_path }}"
              timeout: 900
              environment:
                RUST_LOG: "debug"
        headers:
          Content-Type: "application/json"
      register: test_result
      
    - name: Check test results
      fail:
        msg: "Tests failed: {{ test_result.json.result.content[0].text }}"
      when: test_result.json.result.is_error
      
    - name: Build release via shell tool
      uri:
        url: "{{ sah_server_url }}/tools/call"
        method: POST
        body_format: json
        body:
          jsonrpc: "2.0"
          id: 2
          method: "tools/call"
          params:
            name: "shell_execute"
            arguments:
              command: "cargo build --release"
              working_directory: "{{ project_path }}"
              timeout: 1800
              environment:
                RUSTFLAGS: "-C target-cpu=native"
        headers:
          Content-Type: "application/json"
      register: build_result
      
    - name: Check build results
      fail:
        msg: "Build failed: {{ build_result.json.result.content[0].text }}"
      when: build_result.json.result.is_error
      
    - name: Deploy application
      copy:
        src: "{{ project_path }}/target/release/myapp"
        dest: "/opt/myapp/myapp"
        mode: '0755'
      notify: restart myapp
      
  handlers:
    - name: restart myapp
      systemd:
        name: myapp
        state: restarted
```

### Docker Container Integration

**Docker container that uses shell tool**:
```dockerfile
# Dockerfile.shell-integration
FROM rust:1.70

# Install shell tool client
COPY shell-tool-client /usr/local/bin/
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Set environment variables
ENV SAH_SERVER_URL=http://host.docker.internal:3000
ENV PROJECT_PATH=/workspace

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
```

```bash
#!/bin/bash
# entrypoint.sh

set -e

SAH_SERVER_URL=${SAH_SERVER_URL:-http://localhost:3000}
PROJECT_PATH=${PROJECT_PATH:-/workspace}

# Function to call shell tool
call_shell_tool() {
    local command="$1"
    local timeout="${2:-300}"
    local working_dir="${3:-$PROJECT_PATH}"
    
    curl -X POST "$SAH_SERVER_URL/tools/call" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"id\": 1,
            \"method\": \"tools/call\",
            \"params\": {
                \"name\": \"shell_execute\",
                \"arguments\": {
                    \"command\": \"$command\",
                    \"working_directory\": \"$working_dir\",
                    \"timeout\": $timeout
                }
            }
        }" | jq -r '.result.metadata.stdout'
}

echo "Starting container build process..."

# Format check
echo "Checking code formatting..."
call_shell_tool "cargo fmt -- --check" 60

# Run tests
echo "Running tests..."
call_shell_tool "cargo test" 900

# Build application
echo "Building application..."
call_shell_tool "cargo build --release" 1800

echo "Container build process completed!"
```

## REST API Integration

### HTTP API Wrapper

**Simple HTTP wrapper for shell tool**:
```python
# shell_api_wrapper.py
from flask import Flask, request, jsonify
import requests
import json

app = Flask(__name__)

# Configuration
SAH_SERVER_URL = "http://localhost:3000"

@app.route('/api/shell/execute', methods=['POST'])
def execute_shell_command():
    """HTTP endpoint to execute shell commands."""
    
    data = request.json
    
    # Validate required fields
    if 'command' not in data:
        return jsonify({'error': 'command field is required'}), 400
    
    # Prepare MCP request
    mcp_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "shell_execute",
            "arguments": {
                "command": data['command'],
                "working_directory": data.get('working_directory'),
                "timeout": data.get('timeout', 300),
                "environment": data.get('environment')
            }
        }
    }
    
    try:
        # Call shell tool via MCP
        response = requests.post(
            f"{SAH_SERVER_URL}/tools/call",
            json=mcp_request,
            headers={"Content-Type": "application/json"},
            timeout=30
        )
        response.raise_for_status()
        
        result = response.json()
        
        if 'error' in result:
            return jsonify({'error': f"MCP Error: {result['error']}"}), 500
        
        tool_result = result['result']
        
        return jsonify({
            'success': not tool_result.get('is_error', False),
            'stdout': tool_result['metadata']['stdout'],
            'stderr': tool_result['metadata']['stderr'],
            'exit_code': tool_result['metadata']['exit_code'],
            'execution_time_ms': tool_result['metadata']['execution_time_ms']
        })
        
    except requests.exceptions.RequestException as e:
        return jsonify({'error': f"Request failed: {str(e)}"}), 500
    except Exception as e:
        return jsonify({'error': f"Unexpected error: {str(e)}"}), 500

@app.route('/api/shell/build', methods=['POST'])
def build_project():
    """Convenience endpoint for building projects."""
    
    data = request.json
    project_path = data.get('project_path', '/project')
    build_type = data.get('build_type', 'release')
    
    command = f"cargo build --{build_type}" if build_type == 'release' else "cargo build"
    
    return execute_shell_command_internal({
        'command': command,
        'working_directory': project_path,
        'timeout': 1800,
        'environment': {
            'RUSTFLAGS': '-C target-cpu=native',
            'CARGO_TERM_COLOR': 'always'
        }
    })

def execute_shell_command_internal(params):
    """Internal helper for executing shell commands."""
    
    # Mock request object for reuse of execute_shell_command logic
    class MockRequest:
        def __init__(self, json_data):
            self.json = json_data
    
    original_request = request
    try:
        # Temporarily replace request object
        import flask
        flask.request = MockRequest(params)
        return execute_shell_command()
    finally:
        flask.request = original_request

if __name__ == '__main__':
    app.run(debug=True, port=5000)
```

This integration guide demonstrates how to integrate the shell tool with various systems and platforms. The examples can be adapted for specific use cases and extended with additional functionality as needed.