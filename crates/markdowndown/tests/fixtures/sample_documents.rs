//! Sample documents and data for testing markdowndown functionality.

use std::collections::HashMap;

/// Sample Google Docs export content
pub const GOOGLE_DOCS_MARKDOWN: &str = r#"# Project Specification

This document outlines the requirements for the new project.

## Overview

The goal is to create a comprehensive system that handles:

1. **Data Processing**
   - Input validation
   - Transformation pipelines
   - Output formatting

2. **User Interface**
   - Web-based dashboard
   - Mobile app compatibility
   - Accessibility features

3. **Integration**
   - REST API endpoints
   - Webhook support
   - Third-party service connections

## Technical Requirements

### Backend Services

```python
def process_data(input_data):
    """Process incoming data through validation pipeline."""
    validated = validate_input(input_data)
    transformed = transform_data(validated)
    return format_output(transformed)
```

### Database Schema

| Table | Purpose | Columns |
|-------|---------|---------|
| Users | User management | id, email, created_at |
| Projects | Project tracking | id, name, status, owner_id |
| Tasks | Task management | id, title, description, project_id |

> **Note**: All timestamps should be stored in UTC format.

## Implementation Timeline

- **Phase 1** (Q1): Core infrastructure
- **Phase 2** (Q2): User interface development
- **Phase 3** (Q3): Integration and testing
- **Phase 4** (Q4): Deployment and monitoring

For questions, contact [support@example.com](mailto:support@example.com).
"#;

/// Sample GitHub issue content
pub const GITHUB_ISSUE_MARKDOWN: &str = r#"# Bug Report: Memory Leak in Data Processing

## Description

There appears to be a memory leak in the data processing module that causes the application to consume increasing amounts of RAM over time.

## Steps to Reproduce

1. Start the application
2. Process a large dataset (>100k records)
3. Monitor memory usage over time
4. Observe continuous memory growth

## Expected Behavior

Memory usage should remain stable after initial data loading.

## Actual Behavior

Memory usage increases by approximately 50MB per hour of operation.

## Environment

- **OS**: Ubuntu 20.04
- **Rust Version**: 1.70.0
- **Application Version**: 2.1.3

## Stack Trace

```rust
thread 'main' panicked at 'memory allocation of 2147483648 bytes failed'
src/processor.rs:123:5
```

## Proposed Solution

The issue appears to be related to the `DataCache` struct not properly releasing references to processed items. We should implement proper cleanup in the `process_batch` method.

## Additional Context

This issue was first reported in production after the v2.1.0 release and affects approximately 15% of our user base.
"#;

/// Sample Office 365 document content  
pub const OFFICE365_MARKDOWN: &str = r#"# Meeting Minutes - Q4 Planning Session

**Date**: December 15, 2023  
**Time**: 2:00 PM - 4:00 PM EST  
**Location**: Conference Room A / Virtual  
**Attendees**: 
- Alice Johnson (Product Manager)
- Bob Smith (Engineering Lead)
- Carol Davis (Design Lead)
- David Wilson (Marketing Director)

## Agenda

1. Review Q3 achievements
2. Discuss Q4 objectives
3. Resource allocation
4. Timeline planning

## Q3 Review

### Achievements ✅

- Successfully launched mobile app beta
- Improved API response times by 40%
- Onboarded 3 new team members
- Completed security audit with zero critical findings

### Challenges 🚨

- Delayed integration with third-party payment processor
- Higher than expected bug count in web dashboard
- Resource constraints affected testing timeline

## Q4 Objectives

### Primary Goals

1. **Product Development**
   - Launch mobile app to production
   - Implement advanced analytics dashboard
   - Add multi-language support

2. **Technical Infrastructure** 
   - Migrate to Kubernetes
   - Implement automated deployment pipeline
   - Upgrade database to latest version

3. **Team Growth**
   - Hire 2 senior developers
   - Establish QA process
   - Create comprehensive documentation

### Success Metrics

| Metric | Current | Q4 Target |
|--------|---------|-----------|
| Daily Active Users | 12k | 20k |
| API Uptime | 99.2% | 99.8% |
| Customer Satisfaction | 4.2/5 | 4.5/5 |

## Resource Allocation

### Budget Distribution

- Engineering: 60%
- Design: 20% 
- Marketing: 15%
- Operations: 5%

### Timeline

```mermaid
gantt
    title Q4 Development Timeline
    dateFormat  YYYY-MM-DD
    section Mobile App
    Beta Testing    :2023-10-01, 30d
    Production Launch :2023-11-01, 15d
    section Analytics
    Development     :2023-10-15, 45d
    Testing        :2023-12-01, 15d
```

## Action Items

- [ ] **Alice**: Finalize mobile app feature requirements by Oct 1
- [ ] **Bob**: Set up Kubernetes cluster by Oct 15  
- [ ] **Carol**: Complete new dashboard designs by Oct 30
- [ ] **David**: Launch Q4 marketing campaign by Nov 1
- [ ] **All**: Review and approve budget allocation by Sept 30

## Next Meeting

**Date**: October 1, 2023  
**Time**: 2:00 PM EST  
**Focus**: Mobile app launch preparation

---

*Meeting recorded and notes distributed to all attendees.*
"#;

/// Sample HTML content with complex structure
pub const COMPLEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Advanced Web Development Guide</title>
    <style>
        .highlight { background-color: yellow; }
        .code-block { background-color: #f4f4f4; padding: 10px; }
    </style>
</head>
<body>
    <header>
        <h1>Advanced Web Development Guide</h1>
        <nav>
            <ul>
                <li><a href="#introduction">Introduction</a></li>
                <li><a href="#frameworks">Frameworks</a></li>
                <li><a href="#best-practices">Best Practices</a></li>
            </ul>
        </nav>
    </header>

    <main>
        <section id="introduction">
            <h2>Introduction</h2>
            <p>Web development has evolved significantly over the past decade. Modern applications require <span class="highlight">robust architectures</span> and <strong>scalable solutions</strong>.</p>
            
            <blockquote>
                <p>"The best way to learn web development is by building real projects." - <em>Anonymous Developer</em></p>
            </blockquote>
        </section>

        <section id="frameworks">
            <h2>Popular Frameworks</h2>
            <div class="framework-comparison">
                <h3>Frontend Frameworks</h3>
                <ul>
                    <li><strong>React</strong> - Component-based library by Facebook</li>
                    <li><strong>Vue.js</strong> - Progressive framework for building UIs</li>
                    <li><strong>Angular</strong> - Full-featured framework by Google</li>
                </ul>

                <h3>Backend Frameworks</h3>
                <ol>
                    <li>Express.js (Node.js)</li>
                    <li>Django (Python)</li>
                    <li>Ruby on Rails (Ruby)</li>
                    <li>Spring Boot (Java)</li>
                </ol>

                <table border="1">
                    <thead>
                        <tr>
                            <th>Framework</th>
                            <th>Language</th>
                            <th>Learning Curve</th>
                            <th>Performance</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>React</td>
                            <td>JavaScript</td>
                            <td>Medium</td>
                            <td>High</td>
                        </tr>
                        <tr>
                            <td>Vue.js</td>
                            <td>JavaScript</td>
                            <td>Low</td>
                            <td>High</td>
                        </tr>
                        <tr>
                            <td>Angular</td>
                            <td>TypeScript</td>
                            <td>High</td>
                            <td>High</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </section>

        <section id="best-practices">
            <h2>Best Practices</h2>
            
            <h3>Code Organization</h3>
            <pre class="code-block"><code>project/
├── src/
│   ├── components/
│   ├── services/
│   ├── utils/
│   └── tests/
├── public/
├── docs/
└── package.json</code></pre>

            <h3>Security Considerations</h3>
            <ul>
                <li>Always validate input data</li>
                <li>Use HTTPS for all communications</li>
                <li>Implement proper authentication</li>
                <li>Keep dependencies updated</li>
            </ul>

            <h3>Performance Optimization</h3>
            <p>Key strategies include:</p>
            <dl>
                <dt>Code Splitting</dt>
                <dd>Break your application into smaller chunks that load on demand</dd>
                
                <dt>Lazy Loading</dt>
                <dd>Load resources only when they're needed</dd>
                
                <dt>Caching</dt>
                <dd>Store frequently accessed data for faster retrieval</dd>
            </dl>

            <h4>Example: Async Component Loading</h4>
            <pre class="code-block"><code>const LazyComponent = React.lazy(() => import('./LazyComponent'));

function App() {
  return (
    &lt;Suspense fallback={&lt;div&gt;Loading...&lt;/div&gt;}&gt;
      &lt;LazyComponent /&gt;
    &lt;/Suspense&gt;
  );
}</code></pre>
        </section>

        <aside>
            <h3>Additional Resources</h3>
            <ul>
                <li><a href="https://developer.mozilla.org">MDN Web Docs</a></li>
                <li><a href="https://web.dev">Web.dev by Google</a></li>
                <li><a href="https://css-tricks.com">CSS-Tricks</a></li>
            </ul>
        </aside>
    </main>

    <footer>
        <p>&copy; 2023 Web Development Guide. All rights reserved.</p>
        <p>Contact: <a href="mailto:info@webdevguide.com">info@webdevguide.com</a></p>
    </footer>

    <script>
        console.log("Page loaded successfully");
        // Some JavaScript that should be ignored in markdown conversion
        function handleNavigation() {
            // Navigation logic here
        }
    </script>
</body>
</html>"##;

/// Get sample documents by type
pub fn get_sample_document(doc_type: &str) -> Option<&'static str> {
    match doc_type {
        "google-docs" => Some(GOOGLE_DOCS_MARKDOWN),
        "github-issue" => Some(GITHUB_ISSUE_MARKDOWN),
        "office365" => Some(OFFICE365_MARKDOWN),
        "complex-html" => Some(COMPLEX_HTML),
        _ => None,
    }
}

/// Generate test frontmatter YAML
pub fn generate_test_frontmatter(source_url: &str, exporter: &str) -> String {
    format!(
        r#"---
source_url: "{source_url}"
exporter: "{exporter}"
date_downloaded: "2023-01-01T12:00:00Z"
---"#
    )
}

/// Sample error responses for testing error handling
pub fn get_error_responses() -> HashMap<u16, &'static str> {
    let mut responses = HashMap::new();
    responses.insert(
        400,
        r#"{"error": "Bad Request", "message": "Invalid parameters"}"#,
    );
    responses.insert(
        401,
        r#"{"error": "Unauthorized", "message": "Authentication required"}"#,
    );
    responses.insert(403, r#"{"error": "Forbidden", "message": "Access denied"}"#);
    responses.insert(
        404,
        r#"{"error": "Not Found", "message": "Resource not found"}"#,
    );
    responses.insert(
        429,
        r#"{"error": "Too Many Requests", "message": "Rate limit exceeded"}"#,
    );
    responses.insert(
        500,
        r#"{"error": "Internal Server Error", "message": "Server encountered an error"}"#,
    );
    responses.insert(
        502,
        r#"{"error": "Bad Gateway", "message": "Invalid response from upstream"}"#,
    );
    responses.insert(
        503,
        r#"{"error": "Service Unavailable", "message": "Service temporarily unavailable"}"#,
    );
    responses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_documents_available() {
        assert!(get_sample_document("google-docs").is_some());
        assert!(get_sample_document("github-issue").is_some());
        assert!(get_sample_document("office365").is_some());
        assert!(get_sample_document("complex-html").is_some());
        assert!(get_sample_document("nonexistent").is_none());
    }

    #[test]
    fn test_sample_documents_content() {
        let google_docs = get_sample_document("google-docs").unwrap();
        assert!(google_docs.contains("# Project Specification"));
        assert!(google_docs.contains("Technical Requirements"));

        let github_issue = get_sample_document("github-issue").unwrap();
        assert!(github_issue.contains("# Bug Report"));
        assert!(github_issue.contains("Steps to Reproduce"));

        let office365 = get_sample_document("office365").unwrap();
        assert!(office365.contains("# Meeting Minutes"));
        assert!(office365.contains("Action Items"));
    }

    #[test]
    fn test_frontmatter_generation() {
        let frontmatter = generate_test_frontmatter("https://example.com", "test-exporter");
        assert!(frontmatter.contains("source_url: \"https://example.com\""));
        assert!(frontmatter.contains("exporter: \"test-exporter\""));
        assert!(frontmatter.starts_with("---"));
        assert!(frontmatter.ends_with("---"));
    }

    #[test]
    fn test_error_responses() {
        let errors = get_error_responses();
        assert!(errors.contains_key(&404));
        assert!(errors.contains_key(&500));
        assert!(errors.get(&404).unwrap().contains("Not Found"));
        assert!(errors.get(&500).unwrap().contains("Internal Server Error"));
    }
}
