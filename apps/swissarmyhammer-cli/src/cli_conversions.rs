//! Conversions between CLI argument types and library types.
//!
//! These live in a separate module so that `src/cli.rs` can remain
//! self-contained — depending only on `clap` and `std` — which lets
//! `build.rs` compile `cli.rs` independently via `#[path = "src/cli.rs"]`
//! to generate documentation, man pages, and shell completions at build time.

use crate::cli::SourceArg;

// Re-export FileSource from the common crate so call sites can use
// `crate::cli_conversions::FileSource` alongside the conversion impls below.
pub use swissarmyhammer_common::file_loader::FileSource;

impl From<SourceArg> for FileSource {
    fn from(arg: SourceArg) -> Self {
        match arg {
            SourceArg::Builtin => FileSource::Builtin,
            SourceArg::User => FileSource::User,
            SourceArg::Local => FileSource::Local,
            SourceArg::Dynamic => FileSource::Dynamic,
        }
    }
}

impl From<FileSource> for SourceArg {
    fn from(source: FileSource) -> Self {
        match source {
            FileSource::Builtin => SourceArg::Builtin,
            FileSource::User => SourceArg::User,
            FileSource::Local => SourceArg::Local,
            FileSource::Dynamic => SourceArg::Dynamic,
        }
    }
}

// `From<InstallTarget> for InitScope` is provided by the canonical shared
// `InstallTarget` in `swissarmyhammer_cli_completions::lifecycle`, which
// `crate::cli` re-exports — so it is not redeclared here.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_arg_conversions() {
        // Test From<SourceArg> for FileSource
        assert!(matches!(
            FileSource::from(SourceArg::Builtin),
            FileSource::Builtin
        ));
        assert!(matches!(
            FileSource::from(SourceArg::User),
            FileSource::User
        ));
        assert!(matches!(
            FileSource::from(SourceArg::Local),
            FileSource::Local
        ));
        assert!(matches!(
            FileSource::from(SourceArg::Dynamic),
            FileSource::Dynamic
        ));

        // Test From<FileSource> for SourceArg
        assert!(matches!(
            SourceArg::from(FileSource::Builtin),
            SourceArg::Builtin
        ));
        assert!(matches!(SourceArg::from(FileSource::User), SourceArg::User));
        assert!(matches!(
            SourceArg::from(FileSource::Local),
            SourceArg::Local
        ));
        assert!(matches!(
            SourceArg::from(FileSource::Dynamic),
            SourceArg::Dynamic
        ));
    }
}
