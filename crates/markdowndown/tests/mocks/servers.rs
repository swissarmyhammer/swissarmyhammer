//! Mock HTTP servers for testing external service integrations.

use mockito::{Mock, Server};
use serde_json::json;
use std::collections::HashMap;

/// Timeout duration in seconds for mock server slow response simulation
const MOCK_TIMEOUT_SECONDS: u64 = 5;

/// Rate limit retry duration in seconds for HTTP 429 responses
const RATE_LIMIT_RETRY_SECONDS: &str = "60";

/// HTTP status code for successful requests
const HTTP_OK: usize = 200;

/// HTTP status code for unauthorized requests
const HTTP_UNAUTHORIZED: usize = 401;

/// HTTP status code for not found resources
const HTTP_NOT_FOUND: usize = 404;

/// HTTP status code for rate limit exceeded
const HTTP_TOO_MANY_REQUESTS: usize = 429;

/// Test GitHub issue number for mock data
const TEST_ISSUE_NUMBER: u32 = 123;

/// Test GitHub pull request number for mock data
const TEST_PR_NUMBER: u32 = 456;

/// Generic mock builder that creates a mock with standard configuration
fn create_simple_mock(
    server: &mut Server,
    path: &str,
    status: usize,
    content_type: &str,
    body: &str,
) -> Mock {
    server
        .mock("GET", path)
        .with_status(status)
        .with_header("content-type", content_type)
        .with_body(body)
        .create()
}

/// Generic mock builder that supports multiple headers
fn create_mock_with_headers(
    server: &mut Server,
    path: &str,
    status: usize,
    headers: HashMap<&str, &str>,
    body: &str,
) -> Mock {
    let mut mock = server.mock("GET", path).with_status(status);

    for (key, value) in headers {
        mock = mock.with_header(key, value);
    }

    mock.with_body(body).create()
}

/// Sets up a mock Google Docs export server with specified content type and body
fn setup_google_docs_export_mock(
    server: &mut Server,
    doc_id: &str,
    content_type: &str,
    body: &str,
) -> Mock {
    server
        .mock("GET", &format!("/document/d/{}/export", doc_id))
        .with_status(HTTP_OK)
        .with_header("content-type", content_type)
        .with_body(body)
        .create()
}

/// Sets up a mock Google Docs export server (markdown)
pub fn setup_google_docs_mock(server: &mut Server, doc_id: &str) -> Mock {
    setup_google_docs_export_mock(
        server,
        doc_id,
        "text/markdown",
        crate::helpers::common::SAMPLE_MARKDOWN,
    )
}

/// Sets up a mock Google Docs server that returns HTML
pub fn setup_google_docs_html_mock(server: &mut Server, doc_id: &str) -> Mock {
    setup_google_docs_export_mock(server, doc_id, "text/html", crate::helpers::common::SAMPLE_HTML)
}

/// GitHub entity types for mock setup
enum GitHubEntity {
    Issue,
    Pull,
}

impl GitHubEntity {
    fn path_segment(&self) -> &str {
        match self {
            GitHubEntity::Issue => "issues",
            GitHubEntity::Pull => "pulls",
        }
    }
}

/// Sets up a generic GitHub entity mock (issue or PR)
fn setup_github_entity_mock(
    server: &mut Server,
    entity_type: GitHubEntity,
    owner: &str,
    repo: &str,
    number: u32,
    title: &str,
    body: &str,
    user_login: &str,
) -> Mock {
    let response = json!({
        "number": number,
        "title": title,
        "body": body,
        "user": {
            "login": user_login
        },
        "state": "open",
        "html_url": format!("https://github.com/{}/{}/{}/{}", owner, repo, entity_type.path_segment(), number),
        "created_at": "2023-01-01T12:00:00Z",
        "updated_at": "2023-01-01T12:00:00Z"
    });

    server
        .mock(
            "GET",
            &format!("/repos/{}/{}/{}/{}", owner, repo, entity_type.path_segment(), number),
        )
        .with_status(HTTP_OK)
        .with_header("content-type", "application/json")
        .with_body(response.to_string())
        .create()
}

/// Sets up a mock GitHub API server for issues
pub fn setup_github_api_mock(
    server: &mut Server,
    owner: &str,
    repo: &str,
    number: u32,
) -> Mock {
    setup_github_entity_mock(
        server,
        GitHubEntity::Issue,
        owner,
        repo,
        number,
        "Test Issue",
        "This is a test issue body with **markdown** formatting.",
        "testuser",
    )
}

/// Sets up a mock GitHub API server for pull requests
pub fn setup_github_pr_api_mock(
    server: &mut Server,
    owner: &str,
    repo: &str,
    number: u32,
) -> Mock {
    setup_github_entity_mock(
        server,
        GitHubEntity::Pull,
        owner,
        repo,
        number,
        "Test Pull Request",
        "This is a test PR body with **markdown** formatting.\n\n## Changes\n\n- Added feature A\n- Fixed bug B",
        "contributor",
    )
}

/// Sets up a mock HTML page server
pub fn setup_html_page_mock(server: &mut Server) -> Mock {
    create_simple_mock(
        server,
        "/article.html",
        HTTP_OK,
        "text/html",
        crate::helpers::common::SAMPLE_HTML,
    )
}

/// Sets up a mock server that returns various HTTP error codes
pub fn setup_error_response_mock(server: &mut Server, status_code: usize, path: &str) -> Mock {
    create_simple_mock(
        server,
        path,
        status_code,
        "text/plain",
        &format!("HTTP {status_code} Error"),
    )
}

/// Sets up a mock server that simulates timeout (very slow response)
pub fn setup_timeout_mock(server: &mut Server, path: &str) -> Mock {
    server
        .mock("GET", path)
        .with_status(HTTP_OK)
        .with_header("content-type", "text/html")
        .with_body("Slow response")
        // This will create a very slow response that should timeout
        .with_chunked_body(|w| {
            std::thread::sleep(std::time::Duration::from_secs(MOCK_TIMEOUT_SECONDS));
            w.write_all(b"Delayed response")
        })
        .create()
}

/// Sets up a mock Office 365 server
pub fn setup_office365_mock(server: &mut Server) -> Mock {
    create_simple_mock(
        server,
        "/sites/team/Document.docx",
        HTTP_OK,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "Mock Office 365 document content",
    )
}

/// Authentication mock configuration
struct AuthMockConfig {
    path: String,
    status: usize,
    body: String,
    require_token: Option<String>,
}

/// Sets up an authentication mock with the specified configuration
fn setup_auth_mock(server: &mut Server, config: AuthMockConfig) -> Mock {
    let mut mock = server
        .mock("GET", &config.path)
        .with_status(config.status)
        .with_header("content-type", "application/json");

    if let Some(token) = config.require_token {
        mock = mock.match_header("Authorization", format!("Bearer {token}").as_str());
    }

    mock.with_body(config.body).create()
}

/// Sets up a mock server that requires authentication
pub fn setup_auth_required_mock(server: &mut Server, path: &str) -> Mock {
    setup_auth_mock(
        server,
        AuthMockConfig {
            path: path.to_string(),
            status: HTTP_UNAUTHORIZED,
            body: r#"{"error": "Authentication required"}"#.to_string(),
            require_token: None,
        },
    )
}

/// Sets up a mock server that validates authentication token
pub fn setup_auth_success_mock(server: &mut Server, path: &str, expected_token: &str) -> Mock {
    setup_auth_mock(
        server,
        AuthMockConfig {
            path: path.to_string(),
            status: HTTP_OK,
            body: r#"{"message": "Authenticated successfully"}"#.to_string(),
            require_token: Some(expected_token.to_string()),
        },
    )
}

/// Sets up a mock server that returns rate limiting response
pub fn setup_rate_limit_mock(server: &mut Server, path: &str) -> Mock {
    let mut headers = HashMap::new();
    headers.insert("content-type", "application/json");
    headers.insert("Retry-After", RATE_LIMIT_RETRY_SECONDS);

    create_mock_with_headers(
        server,
        path,
        HTTP_TOO_MANY_REQUESTS,
        headers,
        r#"{"error": "Rate limit exceeded"}"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest;

    /// Helper function to test mock responses
    async fn test_mock_response<F>(
        setup_fn: F,
        path: &str,
        expected_status: u16,
        validate_body: impl FnOnce(String),
    ) where
        F: FnOnce(&mut Server) -> Mock,
    {
        let mut server = mockito::Server::new_async().await;
        let _mock = setup_fn(&mut server);

        let url = format!("{}{}", server.url(), path);
        let response = reqwest::get(&url).await.unwrap();

        assert_eq!(response.status(), expected_status);
        let body = response.text().await.unwrap();
        validate_body(body);
    }

    #[tokio::test]
    async fn test_google_docs_mock() {
        test_mock_response(
            |server| setup_google_docs_mock(server, "abc123"),
            "/document/d/abc123/export",
            HTTP_OK as u16,
            |body| {
                assert!(body.contains("# Test Document"));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_github_api_mock() {
        test_mock_response(
            |server| setup_github_api_mock(server, "owner", "repo", TEST_ISSUE_NUMBER),
            "/repos/owner/repo/issues/123",
            HTTP_OK as u16,
            |body| {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["title"], "Test Issue");
                assert_eq!(json["number"], TEST_ISSUE_NUMBER);
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_error_response_mock() {
        test_mock_response(
            |server| setup_error_response_mock(server, HTTP_NOT_FOUND, "/not-found"),
            "/not-found",
            HTTP_NOT_FOUND as u16,
            |body| {
                assert!(body.contains("HTTP 404 Error"));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_auth_required_mock() {
        test_mock_response(
            |server| setup_auth_required_mock(server, "/protected"),
            "/protected",
            HTTP_UNAUTHORIZED as u16,
            |body| {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["error"], "Authentication required");
            },
        )
        .await;
    }
}
