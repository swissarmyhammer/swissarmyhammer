# LlamaAgent Test Model Configuration

## Test Model Selection

### Small Models for Testing
- Use the smallest viable models for unit and integration tests
- Prefer local models over API-based models for test reliability
- Use consistent model configurations across test environments
- Cache model responses to avoid repeated API calls during testing

### Model Configuration
- Use `llama3.2:1b` or similar small models for test scenarios
- Configure minimal context windows for faster execution
- Set appropriate temperature settings for deterministic testing
- Use shorter max_tokens limits to reduce test execution time

### Test Data Management
- Create realistic but minimal test prompts
- Use deterministic seeds when possible
- Store expected responses for regression testing
- Mock model responses for unit tests when appropriate

## Testing Patterns

### Integration Testing
- Test complete agent workflows with real models
- Verify response parsing and error handling
- Test timeout and retry logic
- Use small datasets to minimize test execution time

### Unit Testing
- Mock the model interface for pure unit tests
- Test prompt generation and response parsing separately
- Focus on business logic, not model behavior
- Use dependency injection for model services

### Performance Testing
- Only create performance tests when explicitly requested
- Use realistic model sizes for performance benchmarks
- Test concurrent agent execution
- Monitor memory usage and cleanup

## Error Handling

### Model Failures
- Test timeout scenarios with slow models
- Handle model unavailability gracefully
- Test malformed response handling
- Implement fallback strategies for model failures

### Rate Limiting
- Respect model API rate limits
- Implement exponential backoff
- Test rate limiting behavior
- Use test-specific rate limit configurations

### Resource Management
- Clean up model resources after tests
- Monitor GPU memory usage if using local models
- Test resource exhaustion scenarios
- Implement proper cleanup in test teardown

## Configuration Management

### Test Environments
- Use separate configurations for test environments
- Override model endpoints for testing
- Use test-specific API keys and credentials
- Isolate test data from production data

### Model Selection
- Allow model override via environment variables
- Provide sane defaults for test environments
- Support multiple model providers
- Document model requirements for tests

### Logging and Debugging
- Log model requests and responses during tests
- Use appropriate log levels for test output
- Include timing information for performance analysis
- Support verbose testing modes for debugging