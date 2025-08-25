This is great example:

<example>
    /// Get the error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            PlanCommandError::FileNotFound { .. } => ErrorSeverity::Error,
            PlanCommandError::PermissionDenied { .. } => ErrorSeverity::Error,
            PlanCommandError::InvalidFileFormat { .. } => ErrorSeverity::Error,
            PlanCommandError::WorkflowExecutionFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::IssueCreationFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::EmptyPlanFile { .. } => ErrorSeverity::Warning,
            PlanCommandError::FileTooLarge { .. } => ErrorSeverity::Error,
            PlanCommandError::IssuesDirectoryNotWritable { .. } => ErrorSeverity::Error,
            PlanCommandError::InsufficientContent { .. } => ErrorSeverity::Warning,
            PlanCommandError::UnsuitableForPlanning { .. } => ErrorSeverity::Warning,
        }
    }
</example>


There should be a shared Severity trait in a new swissarmyhammer-utils crate.

All errors should implement it, take a memo about this.
