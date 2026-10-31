//! The macro that defines the borrowed-string newtypes of the tool registry.

/// Define a public newtype over a borrowed `&str`.
///
/// Each newtype gets the same derives, `AsRef<str>`, `From<&str>` and `Display`, so the
/// string-like behavior is written once for all of them.
macro_rules! str_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name<'a>(pub &'a str);

        impl AsRef<str> for $name<'_> {
            fn as_ref(&self) -> &str {
                self.0
            }
        }

        impl<'a> From<&'a str> for $name<'a> {
            fn from(text: &'a str) -> Self {
                $name(text)
            }
        }

        impl std::fmt::Display for $name<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }
    };
}

pub(super) use str_newtype;
