//! Authentication module for Mirdan CLI.
//!
//! Handles browser-based OAuth login flow with the registry.

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

/// Stored credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub registry: String,
    pub token: String,
    pub email: String,
    pub name: String,
}

/// Token exchange response from registry.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    email: String,
    name: String,
}

/// Callback query parameters.
#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// State shared with callback handler.
struct CallbackState {
    expected_state: String,
    tx: Option<oneshot::Sender<Result<String, String>>>,
}

/// Get the credentials file path.
fn get_credentials_path() -> PathBuf {
    if let Ok(path) = std::env::var("MIRDAN_CREDENTIALS_PATH") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".mirdan")
            .join("credentials")
    }
}

/// Load credentials from file.
pub fn load_credentials() -> Option<Credentials> {
    // First check environment variable for token
    if let Ok(token) = std::env::var("MIRDAN_TOKEN") {
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

/// Save credentials to file with secure permissions.
fn save_credentials(creds: &Credentials) -> std::io::Result<()> {
    let path = get_credentials_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = serde_json::to_string_pretty(creds)?;
    let mut file = fs::File::create(&path)?;
    file.write_all(contents.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

/// Delete credentials file.
fn delete_credentials() -> std::io::Result<()> {
    let path = get_credentials_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Generate a random state string (32 characters).
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

/// Find an available port for the callback server.
fn find_available_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Open URL in default browser.
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

/// Callback handler for OAuth redirect.
async fn handle_callback(
    Query(params): Query<CallbackParams>,
    State(state): State<Arc<tokio::sync::Mutex<CallbackState>>>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    let registry_url = get_registry_url();

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

/// Token verification response.
#[derive(Debug, Deserialize)]
struct VerifyResponse {
    valid: Option<bool>,
    email: Option<String>,
    name: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// User info from verification.
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

/// Run the login flow.
pub async fn login() -> Result<(), RegistryError> {
    if let Some(creds) = load_credentials() {
        if !creds.token.is_empty() {
            if let Ok(info) = verify_token(&creds.token).await {
                println!("Already logged in as {} ({})", info.name, info.email);
                println!("Run 'mirdan logout' first to switch accounts.");
                return Ok(());
            }
        }
    }

    let registry_url = get_registry_url();
    let port = find_available_port()?;
    let state_param = generate_state();

    let (tx, rx) = oneshot::channel();

    let callback_state = Arc::new(tokio::sync::Mutex::new(CallbackState {
        expected_state: state_param.clone(),
        tx: Some(tx),
    }));

    let app = Router::new()
        .route("/callback", get(handle_callback))
        .with_state(callback_state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::AddrInUse, e))?;

    let redirect_uri = format!("http://localhost:{}/callback", port);
    let auth_url = format!(
        "{}/cli/auth/start?redirect_uri={}&state={}",
        registry_url,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state_param)
    );

    println!("Opening browser for authentication...");
    println!("If browser doesn't open, visit: {}", auth_url);

    if let Err(e) = open_browser(&auth_url) {
        eprintln!("Warning: Failed to open browser: {}", e);
        println!("Please open the URL above manually.");
    }

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Waiting for authentication in browser...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

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

/// Run the logout flow.
pub async fn logout() -> Result<(), RegistryError> {
    let creds = load_credentials().ok_or(RegistryError::AuthRequired)?;

    if creds.token.is_empty() {
        return Err(RegistryError::AuthRequired);
    }

    if std::env::var("MIRDAN_TOKEN").is_ok() {
        return Err(RegistryError::Validation(
            "Cannot logout when using MIRDAN_TOKEN environment variable. Unset the variable instead."
                .to_string(),
        ));
    }

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

    if !response.status().is_success() {
        spinner.finish_and_clear();
        eprintln!("Warning: Server token revocation failed, removing local credentials anyway");
    } else {
        spinner.finish_with_message("Token revoked");
    }

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

    let info = verify_token(&creds.token).await?;

    println!("Logged in as: {} ({})", info.name, info.email);
    println!("Registry: {}", creds.registry);

    if std::env::var("MIRDAN_TOKEN").is_ok() {
        println!("(using MIRDAN_TOKEN environment variable)");
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
            token: "mirdan_test123".to_string(),
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
    fn test_find_available_port() {
        let port = find_available_port().unwrap();
        assert!(port > 1024);
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_registry_url_from_env() {
        let original = env::var("MIRDAN_REGISTRY_URL").ok();
        env::set_var("MIRDAN_REGISTRY_URL", "https://custom.example.com");

        let url = get_registry_url();
        assert_eq!(url, "https://custom.example.com");

        match original {
            Some(v) => env::set_var("MIRDAN_REGISTRY_URL", v),
            None => env::remove_var("MIRDAN_REGISTRY_URL"),
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_registry_url_default() {
        let original = env::var("MIRDAN_REGISTRY_URL").ok();
        env::remove_var("MIRDAN_REGISTRY_URL");

        let url = get_registry_url();
        assert_eq!(url, DEFAULT_REGISTRY_URL);

        if let Some(v) = original {
            env::set_var("MIRDAN_REGISTRY_URL", v);
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_credentials_path_from_env() {
        let original = env::var("MIRDAN_CREDENTIALS_PATH").ok();
        env::set_var("MIRDAN_CREDENTIALS_PATH", "/custom/path/creds");

        let path = get_credentials_path();
        assert_eq!(path, PathBuf::from("/custom/path/creds"));

        match original {
            Some(v) => env::set_var("MIRDAN_CREDENTIALS_PATH", v),
            None => env::remove_var("MIRDAN_CREDENTIALS_PATH"),
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_credentials_path_default() {
        let original = env::var("MIRDAN_CREDENTIALS_PATH").ok();
        env::remove_var("MIRDAN_CREDENTIALS_PATH");

        let path = get_credentials_path();
        assert!(path.ends_with(".mirdan/credentials"));

        if let Some(v) = original {
            env::set_var("MIRDAN_CREDENTIALS_PATH", v);
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_load_credentials_from_env_token() {
        let original_token = env::var("MIRDAN_TOKEN").ok();
        let original_path = env::var("MIRDAN_CREDENTIALS_PATH").ok();

        env::set_var("MIRDAN_TOKEN", "mirdan_env_token_123");
        env::set_var("MIRDAN_CREDENTIALS_PATH", "/nonexistent/path");

        let creds = load_credentials();
        assert!(creds.is_some());
        let creds = creds.unwrap();
        assert_eq!(creds.token, "mirdan_env_token_123");

        match original_token {
            Some(v) => env::set_var("MIRDAN_TOKEN", v),
            None => env::remove_var("MIRDAN_TOKEN"),
        }
        match original_path {
            Some(v) => env::set_var("MIRDAN_CREDENTIALS_PATH", v),
            None => env::remove_var("MIRDAN_CREDENTIALS_PATH"),
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_save_and_load_credentials_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let creds_path = temp_dir.path().join("credentials");

        let original = env::var("MIRDAN_CREDENTIALS_PATH").ok();
        env::set_var("MIRDAN_CREDENTIALS_PATH", &creds_path);

        let original_token = env::var("MIRDAN_TOKEN").ok();
        env::remove_var("MIRDAN_TOKEN");

        let creds = Credentials {
            registry: "https://test.example.com".to_string(),
            token: "mirdan_roundtrip_test".to_string(),
            email: "roundtrip@test.com".to_string(),
            name: "Roundtrip Test".to_string(),
        };

        save_credentials(&creds).unwrap();

        let loaded = load_credentials().unwrap();
        assert_eq!(loaded.registry, creds.registry);
        assert_eq!(loaded.token, creds.token);
        assert_eq!(loaded.email, creds.email);
        assert_eq!(loaded.name, creds.name);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&creds_path).unwrap();
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        match original {
            Some(v) => env::set_var("MIRDAN_CREDENTIALS_PATH", v),
            None => env::remove_var("MIRDAN_CREDENTIALS_PATH"),
        }
        match original_token {
            Some(v) => env::set_var("MIRDAN_TOKEN", v),
            None => {} // Already removed
        }
    }

    #[test]
    #[serial(mirdan_env)]
    fn test_delete_credentials() {
        let temp_dir = tempfile::tempdir().unwrap();
        let creds_path = temp_dir.path().join("credentials");

        fs::write(&creds_path, "test").unwrap();
        assert!(creds_path.exists());

        let original = env::var("MIRDAN_CREDENTIALS_PATH").ok();
        env::set_var("MIRDAN_CREDENTIALS_PATH", &creds_path);

        delete_credentials().unwrap();
        assert!(!creds_path.exists());

        match original {
            Some(v) => env::set_var("MIRDAN_CREDENTIALS_PATH", v),
            None => env::remove_var("MIRDAN_CREDENTIALS_PATH"),
        }
    }
}
