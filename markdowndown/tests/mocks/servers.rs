//! Mock HTTP servers for testing external service integrations.

use mockito::{Mock, Server};
use serde_json::json;

/// Sets up a mock Google Docs export server
pub fn setup_google_docs_mock(server: &mut Server) -> Mock {
    server
        .mock("GET", "/document/d/abc123/export")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body(crate::helpers::common::SAMPLE_MARKDOWN)
        .create()
}

/// Sets up a mock Google Docs server that returns HTML
pub fn setup_google_docs_html_mock(server: &mut Server) -> Mock {
    server
        .mock("GET", "/document/d/abc123/export")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(crate::helpers::common::SAMPLE_HTML)
        .create()
}

/// Sets up a mock GitHub API server
pub fn setup_github_api_mock(server: &mut Server) -> Mock {
    let issue_response = json!({
        "number": 123,
        "title": "Test Issue",
        "body": "This is a test issue body with **markdown** formatting.",
        "user": {
            "login": "testuser"
        },
        "state": "open",
        "html_url": "https://github.com/owner/repo/issues/123",
        "created_at": "2023-01-01T12:00:00Z",
        "updated_at": "2023-01-01T12:00:00Z"
    });

    server
        .mock("GET", "/repos/owner/repo/issues/123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(issue_response.to_string())
        .create()
}

/// Sets up a mock GitHub API server for pull requests
pub fn setup_github_pr_api_mock(server: &mut Server) -> Mock {
    let pr_response = json!({
        "number": 456,
        "title": "Test Pull Request",
        "body": "This is a test PR body with **markdown** formatting.\n\n## Changes\n\n- Added feature A\n- Fixed bug B",
        "user": {
            "login": "contributor"
        },
        "state": "open",
        "html_url": "https://github.com/owner/repo/pull/456",
        "created_at": "2023-01-01T12:00:00Z",
        "updated_at": "2023-01-01T12:00:00Z"
    });

    server
        .mock("GET", "/repos/owner/repo/pulls/456")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(pr_response.to_string())
        .create()
}

/// Sets up a mock HTML page server
pub fn setup_html_page_mock(server: &mut Server) -> Mock {
    server
        .mock("GET", "/article.html")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(crate::helpers::common::SAMPLE_HTML)
        .create()
}

/// Sets up a mock server that returns various HTTP error codes
pub fn setup_error_response_mock(server: &mut Server, status_code: usize, path: &str) -> Mock {
    server
        .mock("GET", path)
        .with_status(status_code)
        .with_header("content-type", "text/plain")
        .with_body(format!("HTTP {status_code} Error"))
        .create()
}

/// Sets up a mock server that simulates timeout (very slow response)
pub fn setup_timeout_mock(server: &mut Server, path: &str) -> Mock {
    server
        .mock("GET", path)
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("Slow response")
        // This will create a very slow response that should timeout
        .with_chunked_body(|w| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            w.write_all(b"Delayed response")
        })
        .create()
}

/// Sets up a mock Office 365 server
pub fn setup_office365_mock(server: &mut Server) -> Mock {
    server
        .mock("GET", "/sites/team/Document.docx")
        .with_status(200)
        .with_header(
            "content-type",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )
        .with_body("Mock Office 365 document content")
        .create()
}

/// Sets up a mock server that requires authentication
pub fn setup_auth_required_mock(server: &mut Server, path: &str) -> Mock {
    server
        .mock("GET", path)
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "Authentication required"}"#)
        .create()
}

/// Sets up a mock server that validates authentication token
pub fn setup_auth_success_mock(server: &mut Server, path: &str, expected_token: &str) -> Mock {
    server
        .mock("GET", path)
        .match_header("Authorization", format!("Bearer {expected_token}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message": "Authenticated successfully"}"#)
        .create()
}

/// Sets up a mock server that returns rate limiting response
pub fn setup_rate_limit_mock(server: &mut Server, path: &str) -> Mock {
    server
        .mock("GET", path)
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_header("Retry-After", "60")
        .with_body(r#"{"error": "Rate limit exceeded"}"#)
        .create()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest;

    #[tokio::test]
    async fn test_google_docs_mock() {
        let mut server = mockito::Server::new_async().await;
        let _mock = setup_google_docs_mock(&mut server);

        let url = format!("{}/document/d/abc123/export", server.url());
        let response = reqwest::get(&url).await.unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/markdown"
        );
        let body = response.text().await.unwrap();
        assert!(body.contains("# Test Document"));
    }

    #[tokio::test]
    async fn test_github_api_mock() {
        let mut server = mockito::Server::new_async().await;
        let _mock = setup_github_api_mock(&mut server);

        let url = format!("{}/repos/owner/repo/issues/123", server.url());
        let response = reqwest::get(&url).await.unwrap();

        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["title"], "Test Issue");
        assert_eq!(body["number"], 123);
    }

    #[tokio::test]
    async fn test_error_response_mock() {
        let mut server = mockito::Server::new_async().await;
        let _mock = setup_error_response_mock(&mut server, 404, "/not-found");

        let url = format!("{}/not-found", server.url());
        let response = reqwest::get(&url).await.unwrap();

        assert_eq!(response.status(), 404);
        let body = response.text().await.unwrap();
        assert!(body.contains("HTTP 404 Error"));
    }

    #[tokio::test]
    async fn test_auth_required_mock() {
        let mut server = mockito::Server::new_async().await;
        let _mock = setup_auth_required_mock(&mut server, "/protected");

        let url = format!("{}/protected", server.url());
        let response = reqwest::get(&url).await.unwrap();

        assert_eq!(response.status(), 401);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["error"], "Authentication required");
    }
}
