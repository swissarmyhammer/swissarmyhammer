# Implement HTML to Markdown Conversion

## Overview
Add HTML-to-markdown conversion functionality using the markdowndown crate, transforming fetched HTML content into clean, structured markdown. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Configure markdowndown conversion options (preserve code blocks, tables, links)
- Implement HTML-to-markdown conversion in the tool execution flow
- Extract and preserve metadata (title, description) from HTML
- Handle character encoding detection and conversion
- Clean up unnecessary HTML elements (scripts, styles, ads)

## Implementation Details
- Use `MarkdownOptions` from markdowndown crate for configuration
- Set conversion options: preserve_code_blocks: true, convert_tables: true, preserve_links: true
- Extract HTML title and meta description for response metadata
- Handle different character encodings properly
- Return converted markdown content in response

## Success Criteria
- HTML content is successfully converted to markdown
- Important structural elements (headers, lists, links, code blocks) are preserved
- Metadata is extracted and included in response
- Character encoding is handled correctly
- Clean, readable markdown output

## Dependencies
- Requires fetch_000003_basic-http-client (for HTTP functionality)

## Estimated Impact
- Transforms raw HTML into usable markdown format
- Provides structured content for AI processing