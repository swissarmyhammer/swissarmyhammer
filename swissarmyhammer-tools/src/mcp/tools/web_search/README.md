# Web Search Tools

Privacy-respecting web search functionality using DuckDuckGo with browser automation.

## Requirements

### Chrome/Chromium Browser

The web_search tool requires Chrome or Chromium to be installed on your system. It uses browser automation via [chromiumoxide](https://github.com/mattsse/chromiumoxide) to perform searches.

#### Installation

**macOS:**
```bash
# Via Homebrew
brew install --cask google-chrome
# Or Chromium
brew install --cask chromium
```

**Linux (Ubuntu/Debian):**
```bash
# Chrome
sudo apt install google-chrome-stable
# Or Chromium
sudo apt install chromium-browser
```

**Linux (Fedora):**
```bash
sudo dnf install google-chrome-stable
```

**Windows:**
```powershell
# Via Chocolatey
choco install googlechrome
# Or winget
winget install Google.Chrome
```

#### Verification

Use the `web_search_doctor` tool to verify Chrome is properly installed:

```bash
sah web-search search_doctor
```

With detailed diagnostics:

```bash
sah web-search search_doctor --detailed
```

Expected output when Chrome is properly configured:
```yaml
status: healthy
chrome:
  available: true
  path: /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
  detection_method: standard installation location
recommendations:
  - Chrome is properly configured and ready for web search
```

## Usage

### Basic Web Search

Search without content fetching (faster):

```bash
sah web-search search --query "rust programming" --results_count 5 --fetch_content false
```

Search with content fetching (slower but provides page content):

```bash
sah web-search search --query "rust programming" --results_count 5 --fetch_content true
```

### Via MCP

When used through the MCP protocol, the tool is named `web_search`:

```json
{
  "query": "rust programming language",
  "results_count": 5,
  "fetch_content": false,
  "category": "general",
  "language": "en"
}
```

## Architecture

### Chrome Detection

The web_search tool automatically detects Chrome installation by:

1. Checking the `CHROME` environment variable
2. Searching for Chrome binaries in system PATH
3. Checking platform-specific standard installation locations:
   - macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
   - Linux: `/usr/bin/google-chrome-stable`, `/usr/bin/chromium-browser`, etc.
   - Windows: `Program Files\Google\Chrome\Application\chrome.exe`

Detection logic is in [`chrome_detection.rs`](./chrome_detection.rs).

### Browser Automation

The tool uses chromiumoxide to:
1. Launch a headless Chrome browser with stealth configuration
2. Navigate to DuckDuckGo's HTML search page
3. Extract search results using CSS selectors
4. Parse and score results
5. Optionally fetch and process content from result URLs

### Components

- **`doctor.rs`**: Health check tool for verifying Chrome availability
- **`chrome_detection.rs`**: Cross-platform Chrome detection utility
- **`duckduckgo_client.rs`**: Browser automation for DuckDuckGo searches
- **`search/mod.rs`**: Main MCP tool implementation
- **`content_fetcher.rs`**: Concurrent content fetching from search results
- **`types.rs`**: Shared types and data structures

## Configuration

### Environment Variables

- `CHROME`: Override Chrome path (useful for non-standard installations)
  ```bash
  export CHROME="/path/to/chrome"
  ```

### Search Parameters

- `query` (required): Search query string (max 500 characters)
- `results_count` (optional): Number of results to return (1-50, default: 10)
- `fetch_content` (optional): Whether to fetch page content (default: true)
- `category` (optional): Search category (general, images, videos, news, etc.)
- `language` (optional): Language code (e.g., "en", "en-US")
- `safe_search` (optional): Safe search level (Off, Moderate, Strict)
- `time_range` (optional): Time range filter (day, week, month, year)

## Performance

### Typical Search Times

- **Without content fetching**: 4-6 seconds
- **With content fetching**: 8-15 seconds (depends on number of results and site responsiveness)

### Optimization

The tool uses several optimizations:
- Headless Chrome for faster operation
- HTML version of DuckDuckGo (no JavaScript execution)
- Concurrent content fetching with rate limiting
- Unique temporary browser profiles to avoid lock conflicts

## Troubleshooting

### Chrome Not Found

If you see an error like:
```
Chrome/Chromium browser not found. Web search requires Chrome or Chromium to be installed.
```

1. Install Chrome/Chromium (see Installation section above)
2. Run `sah web-search search_doctor` to verify
3. If Chrome is installed but not detected, set the `CHROME` environment variable

### Browser Launch Failures

If Chrome fails to launch:
- Ensure Chrome/Chromium is up to date
- Check system resources (Chrome requires ~200MB RAM per instance)
- Verify no other processes are holding browser locks

### Known Issues

- **chromiumoxide deserialization errors**: You may see harmless error logs about "data did not match any variant of untagged enum Message". These are known compatibility issues with newer Chrome versions and are handled gracefully.

### Debug Logging

Enable debug logging to see detailed execution:

```bash
RUST_LOG=swissarmyhammer_tools::mcp::tools::web_search=debug sah web-search search --query "test"
```

## Testing

### Unit Tests

Run all web_search unit tests:

```bash
cargo test --package swissarmyhammer-tools --lib web_search
```

### Integration Tests

Integration tests that launch real Chrome are marked with `#[ignore]` by default.

Run Chrome-dependent integration tests:

```bash
cargo test --test tools_tests web_search_integration -- --ignored --nocapture
```

Specific tests:
```bash
# Just verify Chrome detection
cargo test --test tools_tests test_chrome_detection_on_system -- --nocapture

# Test actual web search with Chrome
cargo test --test tools_tests test_web_search_real_chrome -- --ignored --nocapture

# Test with content fetching (slower)
cargo test --test tools_tests test_web_search_with_content -- --ignored --nocapture
```

## Privacy & Security

- **Privacy-respecting**: Uses DuckDuckGo, which doesn't track searches
- **Sandboxed**: Browser runs in sandboxed mode with limited permissions
- **No cookies**: Each search uses a fresh temporary browser profile
- **User agent**: Uses standard Chrome user agent to avoid bot detection
- **No automation detection**: Disables Chrome automation flags

## Limitations

- Requires Chrome/Chromium installation (cannot use other browsers)
- Slower than API-based search due to browser automation overhead
- May encounter CAPTCHA challenges under heavy use (rare)
- Search results limited to what DuckDuckGo provides (no direct API access)

## Future Improvements

Potential enhancements:
- Browser instance pooling for faster repeated searches
- Support for other search engines
- Headless browser alternatives (Firefox, Edge)
- Caching of recent search results
- Parallel search across multiple engines

