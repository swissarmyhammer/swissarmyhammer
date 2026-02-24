//! Chrome browser detection and diagnostics
//!
//! This module provides utilities for detecting Chrome/Chromium installations
//! and providing detailed diagnostics about browser availability.

use std::path::PathBuf;

/// Result of Chrome detection with diagnostic information
#[derive(Debug, Clone)]
pub struct ChromeDetectionResult {
    /// Whether Chrome was found
    pub found: bool,
    /// Path to Chrome executable if found
    pub path: Option<PathBuf>,
    /// List of paths that were checked
    pub paths_checked: Vec<PathBuf>,
    /// Method used to find Chrome (env, path, system)
    pub detection_method: Option<String>,
    /// Diagnostic message
    pub message: String,
}

impl ChromeDetectionResult {
    /// Create a success result
    fn success(path: PathBuf, method: String, paths_checked: Vec<PathBuf>) -> Self {
        Self {
            found: true,
            path: Some(path.clone()),
            paths_checked,
            detection_method: Some(method),
            message: format!("Chrome found at: {}", path.display()),
        }
    }

    /// Create a failure result
    fn failure(paths_checked: Vec<PathBuf>) -> Self {
        let message = format!(
            "Chrome/Chromium not found. Checked {} locations:\n{}",
            paths_checked.len(),
            paths_checked
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Self {
            found: false,
            path: None,
            paths_checked,
            detection_method: None,
            message,
        }
    }

    /// Get installation instructions for the current platform
    pub fn installation_instructions(&self) -> String {
        #[cfg(target_os = "macos")]
        return "Install Chrome via:\n  - Download from https://www.google.com/chrome/\n  - Or use Homebrew: brew install --cask google-chrome\n  - Or use Chromium: brew install --cask chromium".to_string();

        #[cfg(target_os = "linux")]
        return "Install Chrome via:\n  - apt install google-chrome-stable (Debian/Ubuntu)\n  - dnf install google-chrome-stable (Fedora)\n  - Or Chromium: apt install chromium-browser\n  - Or download from https://www.google.com/chrome/".to_string();

        #[cfg(target_os = "windows")]
        return "Install Chrome via:\n  - Download from https://www.google.com/chrome/\n  - Or use Chocolatey: choco install googlechrome\n  - Or use winget: winget install Google.Chrome".to_string();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return "Install Chrome or Chromium from https://www.chromium.org/getting-involved/download-chromium/".to_string();
    }
}

/// Detect Chrome installation with detailed diagnostics
pub fn detect_chrome() -> ChromeDetectionResult {
    let mut paths_checked = Vec::new();

    // 1. Check CHROME environment variable
    if let Ok(chrome_path) = std::env::var("CHROME") {
        let path = PathBuf::from(&chrome_path);
        paths_checked.push(path.clone());
        if path.exists() {
            return ChromeDetectionResult::success(
                path,
                "environment variable CHROME".to_string(),
                paths_checked,
            );
        }
    }

    // 2. Check common binary names in PATH
    let binary_names = [
        "chrome",
        "google-chrome",
        "google-chrome-stable",
        "google-chrome-beta",
        "chromium",
        "chromium-browser",
    ];

    for binary_name in binary_names {
        if let Ok(path) = which::which(binary_name) {
            paths_checked.push(path.clone());
            if path.exists() {
                return ChromeDetectionResult::success(
                    path,
                    format!("PATH ({})", binary_name),
                    paths_checked,
                );
            }
        }
    }

    // 3. Check platform-specific standard installation paths
    let standard_paths = get_standard_chrome_paths();
    for path in standard_paths {
        paths_checked.push(path.clone());
        if path.exists() {
            return ChromeDetectionResult::success(
                path,
                "standard installation location".to_string(),
                paths_checked,
            );
        }
    }

    ChromeDetectionResult::failure(paths_checked)
}

/// Get standard Chrome installation paths for the current platform
fn get_standard_chrome_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta"),
            PathBuf::from("/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev"),
            PathBuf::from(
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            ),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        ]
    }

    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/bin/google-chrome"),
            PathBuf::from("/usr/bin/google-chrome-stable"),
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/usr/bin/chromium-browser"),
            PathBuf::from("/snap/bin/chromium"),
            PathBuf::from("/opt/google/chrome/chrome"),
            PathBuf::from("/opt/chromium.org/chromium/chromium"),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        use std::env;
        let program_files =
            env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let program_files_x86 =
            env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
        let local_appdata = env::var("LOCALAPPDATA").unwrap_or_else(|_| {
            format!(
                "{}\\AppData\\Local",
                env::var("USERPROFILE").unwrap_or_default()
            )
        });

        vec![
            PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                program_files
            )),
            PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                program_files_x86
            )),
            PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                local_appdata
            )),
            PathBuf::from(format!(
                "{}\\Chromium\\Application\\chrome.exe",
                program_files
            )),
            PathBuf::from(format!(
                "{}\\Chromium\\Application\\chrome.exe",
                program_files_x86
            )),
        ]
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        vec![]
    }
}

/// Check if Chrome is available and return the path
pub fn get_chrome_path() -> Option<PathBuf> {
    let result = detect_chrome();
    result.path
}

/// Check if Chrome is available
pub fn is_chrome_available() -> bool {
    detect_chrome().found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chrome() {
        let result = detect_chrome();

        assert!(!result.message.is_empty());
        assert!(!result.paths_checked.is_empty());

        if result.found {
            assert!(result.path.is_some());
            assert!(result.detection_method.is_some());
        } else {
            assert!(result.path.is_none());
            assert!(result.detection_method.is_none());
        }
    }

    #[test]
    fn test_standard_paths_not_empty_on_major_platforms() {
        let paths = get_standard_chrome_paths();

        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        assert!(
            !paths.is_empty(),
            "Standard paths should not be empty on major platforms"
        );
    }

    #[test]
    fn test_installation_instructions() {
        let result = ChromeDetectionResult::failure(vec![]);
        let instructions = result.installation_instructions();

        assert!(!instructions.is_empty());
        assert!(instructions.contains("Install") || instructions.contains("Download"));
    }

    #[test]
    fn test_is_chrome_available() {
        let _available = is_chrome_available();
        // Just verify no panic
    }

    #[test]
    fn test_get_chrome_path() {
        let path = get_chrome_path();

        if let Some(p) = path {
            assert!(p.exists(), "Chrome path should exist if returned");
        }
    }
}
