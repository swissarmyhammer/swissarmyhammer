//! Authentication module for AVP CLI.
//!
//! Handles browser-based OAuth login flow with the AVP registry.

use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::registry::{get_registry_url, RegistryError};

/// Stored credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub registry: String,
    pub token: String,
    pub email: String,
    pub name: String,
}

/// Token exchange response from registry
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    email: String,
    name: String,
}

/// Callback query parameters
#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// State shared with callback handler
struct CallbackState {
    expected_state: String,
    tx: Option<oneshot::Sender<Result<String, String>>>,
}

/// Get the credentials file path
fn get_credentials_path() -> PathBuf {
    if let Ok(path) = std::env::var("AVP_CREDENTIALS_PATH") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".avp")
            .join("credentials")
    }
}

/// Load credentials from file
pub fn load_credentials() -> Option<Credentials> {
    // First check environment variable for token
    if let Ok(token) = std::env::var("AVP_TOKEN") {
        // Return minimal credentials with just the token
        return Some(Credentials {
            registry: get_registry_url(),
            token,
            email: String::new(),
            name: String::new(),
        });
    }

    let path = get_credentials_path();
    if !path.exists() {
        return None;
    }

    let contents = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save credentials to file with secure permissions
fn save_credentials(creds: &Credentials) -> std::io::Result<()> {
    let path = get_credentials_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write credentials
    let contents = serde_json::to_string_pretty(creds)?;
    let mut file = fs::File::create(&path)?;
    file.write_all(contents.as_bytes())?;

    // Set file permissions to 0600 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

/// Delete credentials file
fn delete_credentials() -> std::io::Result<()> {
    let path = get_credentials_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Generate a random state string (32 characters)
fn generate_state() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Find an available port for the callback server
fn find_available_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Open URL in default browser
fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
    }
    Ok(())
}

/// Callback handler for OAuth redirect
async fn handle_callback(
    Query(params): Query<CallbackParams>,
    State(state): State<Arc<tokio::sync::Mutex<CallbackState>>>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    let registry_url = get_registry_url();

    // Check for OAuth error
    if let Some(error) = params.error {
        let msg = params.error_description.unwrap_or_else(|| error.clone());
        if let Some(tx) = state.tx.take() {
            let _ = tx.send(Err(msg.clone()));
        }
        let error_url = format!(
            "{}/cli/error?message={}",
            registry_url,
            urlencoding::encode(&msg)
        );
        return Redirect::to(&error_url).into_response();
    }

    // Validate state parameter
    if let Some(received_state) = &params.state {
        if received_state != &state.expected_state {
            if let Some(tx) = state.tx.take() {
                let _ = tx.send(Err("Invalid state parameter".to_string()));
            }
            let error_url = format!(
                "{}/cli/error?message={}",
                registry_url,
                urlencoding::encode("Invalid state parameter")
            );
            return Redirect::to(&error_url).into_response();
        }
    } else {
        if let Some(tx) = state.tx.take() {
            let _ = tx.send(Err("Missing state parameter".to_string()));
        }
        let error_url = format!(
            "{}/cli/error?message={}",
            registry_url,
            urlencoding::encode("Missing state parameter")
        );
        return Redirect::to(&error_url).into_response();
    }

    // Extract authorization code
    if let Some(code) = params.code {
        if let Some(tx) = state.tx.take() {
            let _ = tx.send(Ok(code));
        }
        let success_url = format!("{}/cli/success", registry_url);
        Redirect::to(&success_url).into_response()
    } else {
        if let Some(tx) = state.tx.take() {
            let _ = tx.send(Err("Missing authorization code".to_string()));
        }
        let error_url = format!(
            "{}/cli/error?message={}",
            registry_url,
            urlencoding::encode("Missing authorization code")
        );
        Redirect::to(&error_url).into_response()
    }
}

/// Run the login flow.
pub async fn login() -> Result<(), RegistryError> {
    // Check if already logged in
    if let Some(creds) = load_credentials() {
        if !creds.token.is_empty() {
            // Verify the existing token
            if let Ok(info) = verify_token(&creds.token).await {
                println!("Already logged in as {} ({})", info.name, info.email);
                println!("Run 'avp logout' first to switch accounts.");
                return Ok(());
            }
            // Token is invalid, proceed with new login
        }
    }

    let registry_url = get_registry_url();

    // Find available port
    let port = find_available_port()?;

    // Generate state for CSRF protection
    let state_param = generate_state();

    // Create channel for receiving callback result
    let (tx, rx) = oneshot::channel();

    // Create callback state
    let callback_state = Arc::new(tokio::sync::Mutex::new(CallbackState {
        expected_state: state_param.clone(),
        tx: Some(tx),
    }));

    // Create callback server
    let app = Router::new()
        .route("/callback", get(handle_callback))
        .with_state(callback_state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::AddrInUse, e))?;

    // Build auth URL
    let redirect_uri = format!("http://localhost:{}/callback", port);
    let auth_url = format!(
        "{}/cli/auth/start?redirect_uri={}&state={}",
        registry_url,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state_param)
    );

    println!("Opening browser for authentication...");
    println!("If browser doesn't open, visit: {}", auth_url);

    // Open browser
    if let Err(e) = open_browser(&auth_url) {
        eprintln!("Warning: Failed to open browser: {}", e);
        println!("Please open the URL above manually.");
    }

    // Start server in background
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    // Show spinner while waiting for callback
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Waiting for authentication in browser...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Wait for callback with timeout
    let code = tokio::select! {
        result = rx => {
            result.map_err(|_| {
                spinner.finish_and_clear();
                RegistryError::Validation("Callback channel closed unexpectedly".to_string())
            })?
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            spinner.finish_and_clear();
            return Err(RegistryError::Validation("Login timed out after 5 minutes".to_string()));
        }
    };

    // Abort the callback server immediately
    server.abort();

    let code = match code {
        Ok(code) => {
            spinner.finish_with_message("Authentication callback received");
            code
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(RegistryError::Unauthorized(e));
        }
    };

    // Exchange code for token
    let exchange_spinner = indicatif::ProgressBar::new_spinner();
    exchange_spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner} {msg}")
            .unwrap(),
    );
    exchange_spinner.set_message("Exchanging authorization code for token...");
    exchange_spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/auth/cli/token", registry_url))
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await?;

    if !response.status().is_success() {
        exchange_spinner.finish_and_clear();
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(RegistryError::Api { status, body });
    }

    let body = response.text().await?;

    let token_response: TokenResponse = serde_json::from_str(&body).map_err(|e| {
        exchange_spinner.finish_and_clear();
        RegistryError::Json(format!(
            "Failed to parse token response: {}. Body: {}",
            e, body
        ))
    })?;

    // Save credentials
    let creds = Credentials {
        registry: registry_url,
        token: token_response.access_token,
        email: token_response.email.clone(),
        name: token_response.name.clone(),
    };

    save_credentials(&creds)?;

    exchange_spinner.finish_and_clear();
    println!(
        "Logged in as {} ({})",
        token_response.name, token_response.email
    );

    Ok(())
}

/// Token verification response
#[derive(Debug, Deserialize)]
struct VerifyResponse {
    valid: Option<bool>,
    email: Option<String>,
    name: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// User info from verification
pub struct UserInfo {
    pub email: String,
    pub name: String,
}

/// Verify token and get user info.
async fn verify_token(token: &str) -> Result<UserInfo, RegistryError> {
    let registry_url = get_registry_url();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/auth/cli/verify", registry_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(RegistryError::Unauthorized(format!(
            "Token verification failed ({}): {}",
            status, body
        )));
    }

    let body = response.text().await?;

    let verify_response: VerifyResponse = serde_json::from_str(&body).map_err(|e| {
        RegistryError::Json(format!(
            "Failed to parse verify response: {}. Body: {}",
            e, body
        ))
    })?;

    // Check if valid field exists and is true, or if we got email/name back
    let is_valid = verify_response.valid.unwrap_or(false)
        || (verify_response.email.is_some() && verify_response.name.is_some());

    if !is_valid {
        return Err(RegistryError::Unauthorized(
            verify_response
                .error_description
                .or(verify_response.error)
                .unwrap_or_else(|| "Token is invalid".to_string()),
        ));
    }

    Ok(UserInfo {
        email: verify_response.email.unwrap_or_default(),
        name: verify_response.name.unwrap_or_default(),
    })
}

/// Run the logout flow.
pub async fn logout() -> Result<(), RegistryError> {
    let creds = load_credentials().ok_or(RegistryError::AuthRequired)?;

    if creds.token.is_empty() {
        return Err(RegistryError::AuthRequired);
    }

    // Check if using environment variable token
    if std::env::var("AVP_TOKEN").is_ok() {
        return Err(RegistryError::Validation(
            "Cannot logout when using AVP_TOKEN environment variable. Unset the variable instead."
                .to_string(),
        ));
    }

    // Revoke token on server
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Revoking token...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = reqwest::Client::new();
    let response = client
        .delete(format!("{}/api/auth/cli/token", creds.registry))
        .header("Authorization", format!("Bearer {}", creds.token))
        .send()
        .await?;

    // Even if server revocation fails, we still delete local credentials
    if !response.status().is_success() {
        spinner.finish_and_clear();
        eprintln!("Warning: Server token revocation failed, removing local credentials anyway");
    } else {
        spinner.finish_with_message("Token revoked");
    }

    // Delete local credentials
    delete_credentials()?;

    println!("Logged out successfully");

    Ok(())
}

/// Run the whoami flow.
pub async fn whoami() -> Result<(), RegistryError> {
    let creds = load_credentials().ok_or(RegistryError::AuthRequired)?;

    if creds.token.is_empty() {
        return Err(RegistryError::AuthRequired);
    }

    // Verify token with server
    let info = verify_token(&creds.token).await?;

    println!("Logged in as: {} ({})", info.name, info.email);
    println!("Registry: {}", creds.registry);

    if std::env::var("AVP_TOKEN").is_ok() {
        println!("(using AVP_TOKEN environment variable)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::DEFAULT_REGISTRY_URL;
    use serial_test::serial;
    use std::env;

    #[test]
    fn test_generate_state() {
        let state = generate_state();
        assert_eq!(state.len(), 32);
        // Should only contain URL-safe characters
        assert!(state
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_generate_state_uniqueness() {
        let state1 = generate_state();
        let state2 = generate_state();
        assert_ne!(state1, state2);
    }

    #[test]
    fn test_credentials_serialization() {
        let creds = Credentials {
            registry: "https://example.com".to_string(),
            token: "avp_test123".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };

        let json = serde_json::to_string(&creds).unwrap();
        let parsed: Credentials = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.registry, creds.registry);
        assert_eq!(parsed.token, creds.token);
        assert_eq!(parsed.email, creds.email);
        assert_eq!(parsed.name, creds.name);
    }

    #[test]
    fn test_credentials_json_format() {
        let creds = Credentials {
            registry: "https://registry.example.com".to_string(),
            token: "avp_abc123".to_string(),
            email: "user@example.com".to_string(),
            name: "Test User".to_string(),
        };

        let json = serde_json::to_string_pretty(&creds).unwrap();
        assert!(json.contains("\"registry\""));
        assert!(json.contains("\"token\""));
        assert!(json.contains("\"email\""));
        assert!(json.contains("\"name\""));
    }

    #[test]
    fn test_token_response_parsing() {
        let json = r#"{"access_token":"avp_xyz789","email":"test@test.com","name":"Test"}"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "avp_xyz789");
        assert_eq!(response.email, "test@test.com");
        assert_eq!(response.name, "Test");
    }

    #[test]
    fn test_token_response_extra_fields() {
        // Server may return extra fields we don't use
        let json = r#"{"access_token":"avp_xyz","email":"a@b.com","name":"A","extra":"ignored"}"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "avp_xyz");
    }

    #[test]
    fn test_verify_response_valid() {
        let json = r#"{"valid":true,"email":"user@example.com","name":"User Name"}"#;
        let response: VerifyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.valid, Some(true));
        assert_eq!(response.email, Some("user@example.com".to_string()));
        assert_eq!(response.name, Some("User Name".to_string()));
    }

    #[test]
    fn test_verify_response_invalid() {
        let json = r#"{"valid":false,"error":"invalid_token","error_description":"Token revoked"}"#;
        let response: VerifyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.valid, Some(false));
        assert_eq!(
            response.error_description,
            Some("Token revoked".to_string())
        );
    }

    #[test]
    fn test_verify_response_minimal() {
        // Server might return minimal response
        let json = r#"{"valid":true}"#;
        let response: VerifyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.valid, Some(true));
        assert_eq!(response.email, None);
    }

    #[test]
    fn test_callback_params_success() {
        let json = r#"{"code":"abc123","state":"xyz789"}"#;
        let params: CallbackParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.code, Some("abc123".to_string()));
        assert_eq!(params.state, Some("xyz789".to_string()));
        assert_eq!(params.error, None);
    }

    #[test]
    fn test_callback_params_error() {
        let json = r#"{"error":"access_denied","error_description":"User cancelled"}"#;
        let params: CallbackParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.code, None);
        assert_eq!(params.error, Some("access_denied".to_string()));
        assert_eq!(params.error_description, Some("User cancelled".to_string()));
    }

    #[test]
    fn test_find_available_port() {
        let port = find_available_port().unwrap();
        assert!(port > 0);
    }

    #[test]
    fn test_find_available_port_valid_range() {
        let port = find_available_port().unwrap();
        // Port should be non-privileged (>1024 typically)
        assert!(port > 1024);
    }

    // Environment variable tests - serialized to avoid race conditions
    #[test]
    #[serial(avp_env)]
    fn test_registry_url_from_env() {
        let original = env::var("AVP_REGISTRY_URL").ok();
        env::set_var("AVP_REGISTRY_URL", "https://custom.example.com");

        let url = get_registry_url();
        assert_eq!(url, "https://custom.example.com");

        // Restore
        match original {
            Some(v) => env::set_var("AVP_REGISTRY_URL", v),
            None => env::remove_var("AVP_REGISTRY_URL"),
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_registry_url_default() {
        let original = env::var("AVP_REGISTRY_URL").ok();
        env::remove_var("AVP_REGISTRY_URL");

        let url = get_registry_url();
        assert_eq!(url, DEFAULT_REGISTRY_URL);

        // Restore
        if let Some(v) = original {
            env::set_var("AVP_REGISTRY_URL", v);
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_credentials_path_from_env() {
        let original = env::var("AVP_CREDENTIALS_PATH").ok();
        env::set_var("AVP_CREDENTIALS_PATH", "/custom/path/creds");

        let path = get_credentials_path();
        assert_eq!(path, PathBuf::from("/custom/path/creds"));

        // Restore
        match original {
            Some(v) => env::set_var("AVP_CREDENTIALS_PATH", v),
            None => env::remove_var("AVP_CREDENTIALS_PATH"),
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_credentials_path_default() {
        let original = env::var("AVP_CREDENTIALS_PATH").ok();
        env::remove_var("AVP_CREDENTIALS_PATH");

        let path = get_credentials_path();
        assert!(path.ends_with(".avp/credentials"));

        // Restore
        if let Some(v) = original {
            env::set_var("AVP_CREDENTIALS_PATH", v);
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_load_credentials_from_env_token() {
        let original_token = env::var("AVP_TOKEN").ok();
        let original_path = env::var("AVP_CREDENTIALS_PATH").ok();

        env::set_var("AVP_TOKEN", "avp_env_token_123");
        env::set_var("AVP_CREDENTIALS_PATH", "/nonexistent/path");

        let creds = load_credentials();
        assert!(creds.is_some());
        let creds = creds.unwrap();
        assert_eq!(creds.token, "avp_env_token_123");

        // Restore
        match original_token {
            Some(v) => env::set_var("AVP_TOKEN", v),
            None => env::remove_var("AVP_TOKEN"),
        }
        match original_path {
            Some(v) => env::set_var("AVP_CREDENTIALS_PATH", v),
            None => env::remove_var("AVP_CREDENTIALS_PATH"),
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_save_and_load_credentials_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let creds_path = temp_dir.path().join("credentials");

        // Temporarily override credentials path
        let original = env::var("AVP_CREDENTIALS_PATH").ok();
        env::set_var("AVP_CREDENTIALS_PATH", &creds_path);

        // Also clear AVP_TOKEN so it doesn't interfere
        let original_token = env::var("AVP_TOKEN").ok();
        env::remove_var("AVP_TOKEN");

        let creds = Credentials {
            registry: "https://test.example.com".to_string(),
            token: "avp_roundtrip_test".to_string(),
            email: "roundtrip@test.com".to_string(),
            name: "Roundtrip Test".to_string(),
        };

        save_credentials(&creds).unwrap();

        let loaded = load_credentials().unwrap();
        assert_eq!(loaded.registry, creds.registry);
        assert_eq!(loaded.token, creds.token);
        assert_eq!(loaded.email, creds.email);
        assert_eq!(loaded.name, creds.name);

        // Verify file permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&creds_path).unwrap();
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        // Restore
        match original {
            Some(v) => env::set_var("AVP_CREDENTIALS_PATH", v),
            None => env::remove_var("AVP_CREDENTIALS_PATH"),
        }
        match original_token {
            Some(v) => env::set_var("AVP_TOKEN", v),
            None => {} // Already removed
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_delete_credentials() {
        let temp_dir = tempfile::tempdir().unwrap();
        let creds_path = temp_dir.path().join("credentials");

        // Create a file first
        fs::write(&creds_path, "test").unwrap();
        assert!(creds_path.exists());

        let original = env::var("AVP_CREDENTIALS_PATH").ok();
        env::set_var("AVP_CREDENTIALS_PATH", &creds_path);

        delete_credentials().unwrap();
        assert!(!creds_path.exists());

        // Restore
        match original {
            Some(v) => env::set_var("AVP_CREDENTIALS_PATH", v),
            None => env::remove_var("AVP_CREDENTIALS_PATH"),
        }
    }

    #[test]
    #[serial(avp_env)]
    fn test_delete_credentials_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let creds_path = temp_dir.path().join("nonexistent");

        let original = env::var("AVP_CREDENTIALS_PATH").ok();
        env::set_var("AVP_CREDENTIALS_PATH", &creds_path);

        // Should not error when file doesn't exist
        delete_credentials().unwrap();

        // Restore
        match original {
            Some(v) => env::set_var("AVP_CREDENTIALS_PATH", v),
            None => env::remove_var("AVP_CREDENTIALS_PATH"),
        }
    }
}
