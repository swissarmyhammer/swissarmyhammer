//! Advanced security hardening for shell command execution
//!
//! This module provides additional security measures beyond basic validation,
//! including advanced threat detection, security policy enforcement, and comprehensive auditing.

use crate::security::{
    ShellAuditEvent, ShellSecurityError, ShellSecurityPolicy, ShellSecurityValidator,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};

/// Advanced security configuration for shell command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHardeningConfig {
    /// Enable advanced threat detection
    pub enable_threat_detection: bool,

    /// Enable process behavior monitoring
    pub enable_behavior_monitoring: bool,

    /// Enable network activity monitoring
    pub enable_network_monitoring: bool,

    /// Enable file system access monitoring
    pub enable_filesystem_monitoring: bool,

    /// Maximum number of processes a command can spawn
    pub max_process_count: u32,

    /// Maximum network connections allowed
    pub max_network_connections: u32,

    /// Maximum file descriptors allowed
    pub max_file_descriptors: u32,

    /// Directories that are strictly forbidden (even for allowed users)
    pub forbidden_directories: Vec<PathBuf>,

    /// File extensions that are forbidden to execute
    pub forbidden_extensions: Vec<String>,

    /// Commands that require additional approval/confirmation
    pub high_risk_commands: Vec<String>,

    /// Enable sandboxing (if available on platform)
    pub enable_sandboxing: bool,


}

impl Default for SecurityHardeningConfig {
    fn default() -> Self {
        Self {
            enable_threat_detection: true,
            enable_behavior_monitoring: true,
            enable_network_monitoring: false, // May be resource intensive
            enable_filesystem_monitoring: true,
            max_process_count: 10,
            max_network_connections: 5,
            max_file_descriptors: 100,
            forbidden_directories: vec![
                PathBuf::from("/proc"),
                PathBuf::from("/sys"),
                PathBuf::from("/dev"),
                PathBuf::from("/boot"),
                PathBuf::from("/root"),
            ],
            forbidden_extensions: vec![
                ".exe".to_string(),
                ".bat".to_string(),
                ".cmd".to_string(),
                ".scr".to_string(),
                ".pif".to_string(),
            ],
            high_risk_commands: vec![
                "dd".to_string(),
                "rm".to_string(),
                "format".to_string(),
                "fdisk".to_string(),
                "mkfs".to_string(),
                "mount".to_string(),
                "umount".to_string(),
                "sudo".to_string(),
                "su".to_string(),
            ],
            enable_sandboxing: false, // Disabled by default due to complexity
        }
    }
}

/// Threat detection patterns and heuristics
#[derive(Debug, Clone)]
pub struct ThreatDetector {
    /// Known malicious command patterns
    malicious_patterns: Vec<regex::Regex>,

    /// Suspicious behavior patterns
    suspicious_patterns: Vec<regex::Regex>,

    /// Command frequency tracking for anomaly detection
    command_frequency: HashMap<String, CommandFrequency>,

    /// Configuration for threat detection
    config: ThreatDetectionConfig,
}

/// Configuration for threat detection
#[derive(Debug, Clone)]
pub struct ThreatDetectionConfig {
    /// Maximum number of commands to keep in history
    pub max_history_size: usize,

    /// Time window for frequency analysis
    pub frequency_window: Duration,

    /// Threshold for suspicious frequency
    pub suspicious_frequency_threshold: u32,

    /// Enable machine learning-based detection (if available)
    pub enable_ml_detection: bool,
}

impl Default for ThreatDetectionConfig {
    fn default() -> Self {
        Self {
            max_history_size: 1000,
            frequency_window: Duration::from_secs(60 * 60), // 60 minutes
            suspicious_frequency_threshold: 50,
            enable_ml_detection: false,
        }
    }
}

/// Command frequency tracking
#[derive(Debug, Clone)]
struct CommandFrequency {
    count: u32,
    first_seen: SystemTime,
    last_seen: SystemTime,
}

/// Security threat levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThreatLevel {
    /// No threat detected
    None,
    /// Low-level suspicious activity
    Low,
    /// Medium-level threat
    Medium,
    /// High-level threat requiring immediate attention
    High,
    /// Critical security threat
    Critical,
}

/// Security assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAssessment {
    /// Overall threat level
    pub threat_level: ThreatLevel,

    /// Specific threats detected
    pub threats: Vec<DetectedThreat>,

    /// Security recommendations
    pub recommendations: Vec<String>,

    /// Whether the command should be allowed
    pub allow_execution: bool,

    /// Additional security measures to apply
    pub required_measures: Vec<SecurityMeasure>,
}

/// Specific security threat detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedThreat {
    /// Type of threat
    pub threat_type: ThreatType,

    /// Severity level
    pub severity: ThreatLevel,

    /// Description of the threat
    pub description: String,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,

    /// Mitigation suggestions
    pub mitigation: Vec<String>,
}

/// Types of security threats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatType {
    /// Command injection attempt
    CommandInjection,

    /// Privilege escalation attempt
    PrivilegeEscalation,

    /// File system manipulation
    FileSystemManipulation,

    /// Network-based attack
    NetworkAttack,

    /// Resource exhaustion attempt
    ResourceExhaustion,

    /// Data exfiltration attempt
    DataExfiltration,

    /// Suspicious frequency patterns
    AnomalousFrequency,

    /// Unknown or generic threat
    Generic,
}

/// Security measures to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityMeasure {
    /// Require additional confirmation
    RequireConfirmation,

    /// Enable additional monitoring
    EnableMonitoring,

    /// Apply resource limits
    ApplyResourceLimits,

    /// Run in sandbox
    RunInSandbox,

    /// Require elevated privileges
    RequireElevatedPrivileges,

    /// Log all activities
    EnableFullLogging,
}

impl Default for ThreatDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreatDetector {
    /// Create a new threat detector
    pub fn new() -> Self {
        Self::with_config(ThreatDetectionConfig::default())
    }

    /// Create a new threat detector with custom configuration
    pub fn with_config(config: ThreatDetectionConfig) -> Self {
        let malicious_patterns = vec![
            // Command injection patterns
            regex::Regex::new(r"[;&|`$()]").unwrap(),
            regex::Regex::new(r"\.\.\/").unwrap(), // Path traversal
            regex::Regex::new(r"\/etc\/passwd|\/etc\/shadow").unwrap(), // Sensitive file access
            regex::Regex::new(r"curl\s+.*\|\s*sh|wget\s+.*\|\s*sh").unwrap(), // Download and execute
            regex::Regex::new(r"nc\s+-l|netcat\s+-l").unwrap(), // Reverse shell attempts
            regex::Regex::new(r"rm\s+-rf\s+\/").unwrap(),       // Destructive operations
        ];

        let suspicious_patterns = vec![
            // Suspicious but not necessarily malicious
            regex::Regex::new(r"base64|xxd|hexdump").unwrap(), // Encoding/decoding tools
            regex::Regex::new(r"ssh\s+|scp\s+|rsync\s+").unwrap(), // Network file transfer
            regex::Regex::new(r"find\s+.*-exec").unwrap(),     // Find with execute
            regex::Regex::new(r"grep\s+-r|egrep\s+-r").unwrap(), // Recursive searches
        ];

        Self {
            malicious_patterns,
            suspicious_patterns,
            command_frequency: HashMap::new(),

            config,
        }
    }

    /// Analyze a command for security threats
    pub fn analyze_command(
        &mut self,
        command: &str,
        _context: &CommandContext,
    ) -> SecurityAssessment {
        let mut threats = Vec::new();
        let mut threat_level = ThreatLevel::None;
        let mut recommendations = Vec::new();
        let mut required_measures = Vec::new();

        // Check for malicious patterns
        for pattern in &self.malicious_patterns {
            if pattern.is_match(command) {
                let threat = DetectedThreat {
                    threat_type: ThreatType::CommandInjection,
                    severity: ThreatLevel::High,
                    description: format!("Command matches malicious pattern: {}", pattern.as_str()),
                    confidence: 0.8,
                    mitigation: vec![
                        "Review command for injection attempts".to_string(),
                        "Consider using safer alternatives".to_string(),
                    ],
                };
                threats.push(threat);
                threat_level = threat_level.max(ThreatLevel::High);
            }
        }

        // Check for suspicious patterns
        for pattern in &self.suspicious_patterns {
            if pattern.is_match(command) {
                let threat = DetectedThreat {
                    threat_type: ThreatType::Generic,
                    severity: ThreatLevel::Medium,
                    description: format!(
                        "Command matches suspicious pattern: {}",
                        pattern.as_str()
                    ),
                    confidence: 0.6,
                    mitigation: vec![
                        "Monitor execution closely".to_string(),
                        "Enable additional logging".to_string(),
                    ],
                };
                threats.push(threat);
                threat_level = threat_level.max(ThreatLevel::Medium);
                required_measures.push(SecurityMeasure::EnableMonitoring);
            }
        }

        // Analyze command frequency
        self.update_command_frequency(command);
        if let Some(frequency_threat) = self.check_frequency_anomalies(command) {
            threats.push(frequency_threat);
            threat_level = threat_level.max(ThreatLevel::Medium);
        }

        // Check for privilege escalation attempts
        if self.detect_privilege_escalation(command) {
            let threat = DetectedThreat {
                threat_type: ThreatType::PrivilegeEscalation,
                severity: ThreatLevel::High,
                description: "Command appears to attempt privilege escalation".to_string(),
                confidence: 0.7,
                mitigation: vec![
                    "Verify user authorization for privileged operations".to_string(),
                    "Consider requiring additional confirmation".to_string(),
                ],
            };
            threats.push(threat);
            threat_level = threat_level.max(ThreatLevel::High);
            required_measures.push(SecurityMeasure::RequireConfirmation);
        }

        // Check for resource exhaustion attempts
        if self.detect_resource_exhaustion(command) {
            let threat = DetectedThreat {
                threat_type: ThreatType::ResourceExhaustion,
                severity: ThreatLevel::Medium,
                description: "Command may consume excessive system resources".to_string(),
                confidence: 0.6,
                mitigation: vec![
                    "Apply strict resource limits".to_string(),
                    "Monitor resource usage during execution".to_string(),
                ],
            };
            threats.push(threat);
            threat_level = threat_level.max(ThreatLevel::Medium);
            required_measures.push(SecurityMeasure::ApplyResourceLimits);
        }

        // Generate recommendations based on threats
        if threat_level >= ThreatLevel::High {
            recommendations.push("Consider blocking this command execution".to_string());
            recommendations.push("Conduct security review before proceeding".to_string());
        } else if threat_level >= ThreatLevel::Medium {
            recommendations.push("Enable enhanced monitoring for this execution".to_string());
            recommendations.push("Review command parameters carefully".to_string());
        }

        SecurityAssessment {
            threat_level,
            threats,
            recommendations,
            allow_execution: threat_level < ThreatLevel::Critical,
            required_measures,
        }
    }

    /// Update command frequency tracking
    fn update_command_frequency(&mut self, command: &str) {
        let now = SystemTime::now();
        let frequency = self
            .command_frequency
            .entry(command.to_string())
            .or_insert_with(|| CommandFrequency {
                count: 0,
                first_seen: now,
                last_seen: now,
            });

        frequency.count += 1;
        frequency.last_seen = now;
    }

    /// Check for frequency anomalies
    fn check_frequency_anomalies(&self, command: &str) -> Option<DetectedThreat> {
        if let Some(frequency) = self.command_frequency.get(command) {
            let time_window = SystemTime::now()
                .duration_since(frequency.first_seen)
                .unwrap_or(Duration::ZERO);

            if time_window < self.config.frequency_window
                && frequency.count > self.config.suspicious_frequency_threshold
            {
                return Some(DetectedThreat {
                    threat_type: ThreatType::AnomalousFrequency,
                    severity: ThreatLevel::Medium,
                    description: format!(
                        "Command executed {} times in {} seconds (threshold: {})",
                        frequency.count,
                        time_window.as_secs(),
                        self.config.suspicious_frequency_threshold
                    ),
                    confidence: 0.7,
                    mitigation: vec![
                        "Investigate reason for high frequency execution".to_string(),
                        "Consider rate limiting this command".to_string(),
                    ],
                });
            }
        }
        None
    }

    /// Detect privilege escalation attempts
    fn detect_privilege_escalation(&self, command: &str) -> bool {
        let escalation_patterns = [
            "sudo",
            "su",
            "doas",
            "runuser",
            "pkexec",
            "chmod +s",
            "chown root",
            "setuid",
            "setgid",
        ];

        escalation_patterns
            .iter()
            .any(|&pattern| command.contains(pattern))
    }

    /// Detect potential resource exhaustion attempts
    fn detect_resource_exhaustion(&self, command: &str) -> bool {
        let exhaustion_patterns = [
            ":(){ :|:& };:", // Fork bomb
            "/dev/zero",
            "dd if=/dev/zero",
            "while true;",
            "for ((;;))",
            "cat /dev/urandom",
        ];

        exhaustion_patterns
            .iter()
            .any(|&pattern| command.contains(pattern))
    }

    /// Get security statistics
    pub fn get_security_statistics(&self) -> SecurityStatistics {
        let total_commands = 0; // Command history tracking was removed as dead code
        let unique_commands = self.command_frequency.len();

        let high_frequency_commands = self
            .command_frequency
            .iter()
            .filter(|(_, freq)| freq.count > self.config.suspicious_frequency_threshold)
            .count();

        SecurityStatistics {
            total_commands_analyzed: total_commands,
            unique_commands,
            high_frequency_commands,
            threat_detection_enabled: true,
            last_analysis_time: SystemTime::now(),
        }
    }
}

/// Context information for command analysis
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Source IP address (if applicable)
    pub source_ip: Option<String>,

    /// User ID executing the command
    pub user_id: Option<String>,

    /// Working directory
    pub working_directory: PathBuf,

    /// Environment variables
    pub environment: HashMap<String, String>,

    /// Process ID of parent process
    pub parent_pid: Option<u32>,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            source_ip: None,
            user_id: None,
            working_directory: PathBuf::from("."),
            environment: HashMap::new(),
            parent_pid: None,
        }
    }
}

/// Security statistics and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityStatistics {
    /// Total number of commands analyzed
    pub total_commands_analyzed: usize,

    /// Number of unique commands seen
    pub unique_commands: usize,

    /// Number of high-frequency commands
    pub high_frequency_commands: usize,

    /// Whether threat detection is enabled
    pub threat_detection_enabled: bool,

    /// Last analysis timestamp
    pub last_analysis_time: SystemTime,
}

/// Enhanced security validator with hardening features
#[derive(Debug)]
pub struct HardenedSecurityValidator {
    /// Base command validator
    base_validator: &'static ShellSecurityValidator,

    /// Threat detector
    threat_detector: ThreatDetector,

    /// Security hardening configuration
    hardening_config: SecurityHardeningConfig,
}

impl HardenedSecurityValidator {
    /// Create a new hardened security validator
    pub fn new(_policy: ShellSecurityPolicy, hardening_config: SecurityHardeningConfig) -> Self {
        Self {
            base_validator: crate::security::get_validator(),
            threat_detector: ThreatDetector::new(),
            hardening_config,
        }
    }

    /// Perform comprehensive security validation
    pub fn validate_command_comprehensive(
        &mut self,
        command: &str,
        working_dir: &Path,
        environment: &HashMap<String, String>,
        context: CommandContext,
    ) -> Result<SecurityAssessment, ShellSecurityError> {
        // First, run basic validation
        self.base_validator.validate_command(command)?;
        self.base_validator.validate_directory_access(working_dir)?;
        self.base_validator
            .validate_environment_variables(environment)?;

        // Then run threat detection if enabled
        if self.hardening_config.enable_threat_detection {
            let assessment = self.threat_detector.analyze_command(command, &context);

            // Log security assessment
            match assessment.threat_level {
                ThreatLevel::Critical => {
                    error!("Critical security threat detected: {:?}", assessment)
                }
                ThreatLevel::High => warn!("High security threat detected: {:?}", assessment),
                ThreatLevel::Medium => info!("Medium security threat detected: {:?}", assessment),
                ThreatLevel::Low => debug!("Low security threat detected: {:?}", assessment),
                ThreatLevel::None => debug!("No security threats detected"),
            }

            // Create audit event for security assessment
            let audit_event = ShellAuditEvent {
                timestamp: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                command: command.to_string(),
                working_directory: Some(working_dir.to_path_buf()),
                environment_vars: environment.clone(),
                exit_code: None,
                execution_time_ms: None,
                validation_result: if assessment.allow_execution {
                    "PASSED".to_string()
                } else {
                    format!("FAILED: {:?}", assessment.threats)
                },
                security_policy_version: "hardened-v1".to_string(),
            };

            // Log audit event (this would integrate with the existing audit system)
            debug!("Security audit event: {:?}", audit_event);

            return Ok(assessment);
        }

        // If threat detection is disabled, return a basic assessment
        Ok(SecurityAssessment {
            threat_level: ThreatLevel::None,
            threats: Vec::new(),
            recommendations: Vec::new(),
            allow_execution: true,
            required_measures: Vec::new(),
        })
    }

    /// Get security statistics from the validator
    pub fn get_security_statistics(&self) -> SecurityStatistics {
        self.threat_detector.get_security_statistics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_detector_creation() {
        let detector = ThreatDetector::new();
        assert!(!detector.malicious_patterns.is_empty());
        assert!(!detector.suspicious_patterns.is_empty());
    }

    #[test]
    fn test_malicious_pattern_detection() {
        let mut detector = ThreatDetector::new();
        let context = CommandContext::default();

        // Test command injection
        let assessment = detector.analyze_command("echo test; rm -rf /", &context);
        assert!(assessment.threat_level >= ThreatLevel::High);
        assert!(!assessment.threats.is_empty());

        // Test safe command
        let assessment = detector.analyze_command("echo hello", &context);
        assert_eq!(assessment.threat_level, ThreatLevel::None);
        assert!(assessment.threats.is_empty());
    }

    #[test]
    fn test_privilege_escalation_detection() {
        let mut detector = ThreatDetector::new();
        let context = CommandContext::default();

        let assessment = detector.analyze_command("sudo rm file", &context);
        assert!(assessment.threat_level >= ThreatLevel::High);
        assert!(assessment
            .threats
            .iter()
            .any(|t| matches!(t.threat_type, ThreatType::PrivilegeEscalation)));
    }

    #[test]
    fn test_resource_exhaustion_detection() {
        let mut detector = ThreatDetector::new();
        let context = CommandContext::default();

        let assessment = detector.analyze_command(":(){ :|:& };:", &context);
        assert!(assessment.threat_level >= ThreatLevel::Medium);
        assert!(assessment
            .threats
            .iter()
            .any(|t| matches!(t.threat_type, ThreatType::ResourceExhaustion)));
    }

    #[test]
    fn test_frequency_analysis() {
        let mut detector = ThreatDetector::new();
        let context = CommandContext::default();

        // Execute the same command multiple times rapidly
        for _ in 0..60 {
            let assessment = detector.analyze_command("echo test", &context);
            if assessment
                .threats
                .iter()
                .any(|t| matches!(t.threat_type, ThreatType::AnomalousFrequency))
            {
                break;
            }
        }

        // Should detect frequency anomaly
        let assessment = detector.analyze_command("echo test", &context);
        assert!(assessment
            .threats
            .iter()
            .any(|t| matches!(t.threat_type, ThreatType::AnomalousFrequency)));
    }

    #[test]
    fn test_security_assessment_serialization() {
        let assessment = SecurityAssessment {
            threat_level: ThreatLevel::High,
            threats: vec![DetectedThreat {
                threat_type: ThreatType::CommandInjection,
                severity: ThreatLevel::High,
                description: "Test threat".to_string(),
                confidence: 0.8,
                mitigation: vec!["Test mitigation".to_string()],
            }],
            recommendations: vec!["Test recommendation".to_string()],
            allow_execution: false,
            required_measures: vec![SecurityMeasure::RequireConfirmation],
        };

        let json = serde_json::to_string(&assessment).unwrap();
        let deserialized: SecurityAssessment = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.threat_level, ThreatLevel::High);
        assert!(!deserialized.allow_execution);
    }
}
