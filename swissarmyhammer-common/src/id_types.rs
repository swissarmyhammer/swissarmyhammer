//! The `define_id!` macro for creating ULID-based newtype wrappers.
//!
//! This macro lives in `swissarmyhammer-common` so every crate in the ecosystem
//! can use it without pulling in heavier dependencies. The macro is
//! `#[macro_export]`-ed, so downstream crates import it as
//! `use swissarmyhammer_common::define_id;`.

/// Macro to define ID/name newtypes with consistent derives and impls.
///
/// Each newtype wraps a `String` and provides:
/// - `#[serde(transparent)]` for seamless YAML/JSON (de)serialization
/// - `Display`, `AsRef<str>`, `From<&str>`, `From<String>`
/// - `new()` generates a fresh ULID (useful for ID types)
/// - `from_string()` wraps an existing string
/// - `as_str()` borrows the inner value
///
/// # Example
///
/// ```rust,ignore
/// define_id!(TaskId, "ULID-based identifier for tasks");
/// let id = TaskId::new();  // fresh ULID
/// let id2 = TaskId::from_string("my-slug");
/// assert_eq!(id2.as_str(), "my-slug");
/// ```
#[macro_export]
macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Create a new ID with a fresh ULID.
            pub fn new() -> Self {
                Self(ulid::Ulid::new().to_string())
            }

            /// Create an ID from an existing string.
            pub fn from_string(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Get the inner string value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                self.0 == *other
            }
        }

        impl std::str::FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                Ok(Self(s.to_string()))
            }
        }
    };
}
