# Troubleshooting

This guide helps you diagnose and resolve common issues with SwissArmyHammer Tools.

## Installation Issues

### Cargo Install Fails

**Symptom:** `cargo install swissarmyhammer` fails with compilation errors.

**Solutions:**
1. Update Rust: `rustup update`
2. Check Rust version: `rustc --version` (need 1.70+)
3. Clear cargo cache: `cargo clean` in the project directory
4. Try with verbose output: `cargo install swissarmyhammer -v`

### Missing Dependencies

**Symptom:** Compilation fails with missing system dependencies.

**Solutions:**

On macOS:
```bash
xcode-select --install
```

On Ubuntu/Debian:
```bash
sudo apt-get install build-essential pkg-config libssl-dev
```

On Fedora/RHEL:
```bash
sudo dnf install gcc openssl-devel
```

## Server Issues

### Server Won't Start

**Symptom:** `sah serve` fails to start.

**Solutions:**
1. Check working directory exists: `pwd`
2. Verify permissions: `ls -la`
3. Check for port conflicts (HTTP mode): `lsof -i :3000`
4. Review error messages carefully

### Server Crashes

**Symptom:** Server starts but crashes immediately.

**Solutions:**
1. Check available disk space: `df -h`
2. Verify file permissions in working directory
3. Look for corrupted `.swissarmyhammer/` files
4. Try fresh directory: `rm -rf .swissarmyhammer/`

### Connection Issues

**Symptom:** Client can't connect to server.

**Solutions:**

For stdio mode:
- Verify server is started: check process list
- Check configuration file syntax
- Review client logs for errors

For HTTP mode:
- Verify server is listening: `lsof -i :PORT`
- Check firewall settings
- Test with curl: `curl http://localhost:PORT/`

## Tool Execution Issues

### File Operation Failures

**Symptom:** File operations fail with permission errors.

**Solutions:**
1. Check file permissions: `ls -la FILE`
2. Verify working directory: `pwd`
3. Ensure path is absolute (not relative)
4. Check disk space: `df -h`

**Symptom:** File not found errors.

**Solutions:**
1. Verify file path is correct
2. Check for typos in path
3. Use absolute paths
4. Verify working directory is correct

### Search Issues

**Symptom:** Search returns no results.

**Solutions:**
1. Verify index exists: `ls .swissarmyhammer/search.db`
2. Re-index files: use `search_index` with `force: true`
3. Check glob patterns match your files
4. Try broader search terms
5. Verify files are in supported languages

**Symptom:** Search results are stale.

**Solutions:**
1. Force re-index: `search_index` with `force: true`
2. Check file modification times
3. Delete index and re-create: `rm .swissarmyhammer/search.db`

### Issue Management Problems

**Symptom:** Issues not appearing in list.

**Solutions:**
1. Check `.swissarmyhammer/issues/` exists
2. Verify issue files are `.md` format
3. Check file permissions
4. Try with `show_completed: true` to see all issues

**Symptom:** Can't update or complete issues.

**Solutions:**
1. Verify issue name is correct (case-sensitive)
2. Check file isn't locked by another process
3. Verify disk space available
4. Check file permissions

## Performance Issues

### Slow Indexing

**Symptom:** Search indexing takes very long.

**Solutions:**
1. Reduce patterns: index only necessary files
2. Exclude large generated files
3. Check for very large files
4. Verify tree-sitter parsers are installed
5. Monitor disk I/O

### Slow Queries

**Symptom:** Search queries are slow.

**Solutions:**
1. Reduce result limit
2. Check database size: `ls -lh .swissarmyhammer/search.db`
3. Try more specific queries
4. Re-index to optimize database

### High Memory Usage

**Symptom:** Server uses excessive memory.

**Solutions:**
1. Check for very large files being read
2. Use `offset` and `limit` for large file reads
3. Reduce search result limits
4. Monitor indexing of large codebases

## Git Integration Issues

### Changes Not Detected

**Symptom:** `git_changes` doesn't show expected files.

**Solutions:**
1. Verify you're on correct branch: `git branch`
2. Check git status: `git status`
3. Ensure working directory is git repository
4. Verify parent branch exists

### Wrong Parent Branch

**Symptom:** Changes show diff against wrong branch.

**Solutions:**
1. Check branch naming convention (issue/* branches)
2. Verify parent branch reference
3. Use explicit parent branch if needed

## Web Tool Issues

### Fetch Failures

**Symptom:** `web_fetch` fails to retrieve content.

**Solutions:**
1. Verify URL is accessible: `curl URL`
2. Check network connectivity
3. Verify no firewall blocking
4. Try with different timeout value
5. Check for rate limiting

### Search No Results

**Symptom:** `web_search` returns no results.

**Solutions:**
1. Try different search terms
2. Remove category filter
3. Check network connectivity
4. Verify DuckDuckGo is accessible

## Configuration Issues

### Invalid Configuration

**Symptom:** Server fails to start with config errors.

**Solutions:**
1. Validate JSON syntax
2. Check for trailing commas
3. Verify required fields present
4. Review example configurations

### Configuration Not Applied

**Symptom:** Configuration changes not taking effect.

**Solutions:**
1. Restart server after config changes
2. Verify config file location
3. Check file permissions
4. Review logs for config warnings

## Debugging Tips

### Enable Verbose Logging

Set environment variable:
```bash
RUST_LOG=debug sah serve
```

### Check Logs

Review MCP logs (location varies by client):
- Claude Desktop: check application logs
- Custom client: check stdout/stderr

### Test Individual Tools

Use MCP inspector or similar tool to test tools individually.

### Reproduce Minimal Case

Create minimal reproduction:
1. Start with fresh directory
2. Test single operation
3. Add complexity gradually

## Getting Help

### Check Documentation

- [Getting Started](./getting-started.md): Installation and setup
- [Features](./features.md): Tool capabilities and usage
- [Architecture](./architecture.md): System design

### Report Issues

When reporting issues, include:

1. SwissArmyHammer version: `sah --version`
2. Operating system and version
3. Rust version: `rustc --version`
4. Complete error messages
5. Steps to reproduce
6. Expected vs actual behavior

### Community Resources

- GitHub Issues: Report bugs and request features
- Documentation: Browse guides and references
- Examples: Review example configurations

## Common Error Messages

### "Path traversal detected"

**Cause:** Attempting to access files outside working directory.

**Solution:** Use paths within working directory or change working directory.

### "File encoding not supported"

**Cause:** File uses unsupported encoding.

**Solution:** Convert file to UTF-8 or use binary mode.

### "Pattern not found"

**Cause:** Exact string match failed in file edit.

**Solution:** Verify string is exact match, including whitespace.

### "No index found"

**Cause:** Search query without existing index.

**Solution:** Run `search_index` before `search_query`.

### "Issue not found"

**Cause:** Issue name doesn't match any file.

**Solution:** Check issue name (case-sensitive), use `issue_list` to verify.

## Prevention

### Best Practices

1. **Regular Backups**: Commit `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/` to git
2. **Clean State**: Delete `.swissarmyhammer/search.db` when stale
3. **Validate Input**: Check tool parameters before execution
4. **Monitor Resources**: Watch disk space and memory usage
5. **Update Regularly**: Keep SwissArmyHammer updated

### Maintenance

1. **Re-index Periodically**: Force re-index after major refactorings
2. **Clean Completed Issues**: Archive or delete old completed issues
3. **Monitor Database Size**: Check search database growth
4. **Review Logs**: Periodically check for warnings

## Still Having Issues?

If you're still experiencing problems:

1. Review all documentation thoroughly
2. Search existing GitHub issues
3. Create a minimal reproduction case
4. Report issue with complete details
5. Include all diagnostic information

The SwissArmyHammer team is committed to providing excellent support and resolving issues promptly.
