//! Parameter metadata for CLI generation
//!
//! This metadata is derived from struct fields, not duplicated.

/// Parameter type for schema generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
}

/// Metadata about a parameter - derived from struct fields
#[derive(Debug, Clone)]
pub struct ParamMeta {
    /// Field name
    pub name: &'static str,
    /// Description (from doc comment)
    pub description: &'static str,
    /// Parameter type
    pub param_type: ParamType,
    /// Whether required (non-Option field)
    pub required: bool,
    /// CLI short flag
    pub short: Option<char>,
    /// Alternative names
    pub aliases: &'static [&'static str],
}

impl ParamMeta {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            description: "",
            param_type: ParamType::String,
            required: false,
            short: None,
            aliases: &[],
        }
    }

    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub const fn description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    pub const fn short(mut self, c: char) -> Self {
        self.short = Some(c);
        self
    }

    pub const fn aliases(mut self, a: &'static [&'static str]) -> Self {
        self.aliases = a;
        self
    }

    pub const fn param_type(mut self, t: ParamType) -> Self {
        self.param_type = t;
        self
    }

    /// Set short flag from Option (for macro compatibility)
    pub const fn short_opt(mut self, c: Option<char>) -> Self {
        self.short = c;
        self
    }
}
