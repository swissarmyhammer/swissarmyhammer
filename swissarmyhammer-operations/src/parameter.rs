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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let p = ParamMeta::new("my_param");
        assert_eq!(p.name, "my_param");
        assert_eq!(p.description, "");
        assert_eq!(p.param_type, ParamType::String);
        assert!(!p.required);
        assert!(p.short.is_none());
        assert!(p.aliases.is_empty());
    }

    #[test]
    fn test_required_builder() {
        let p = ParamMeta::new("x").required();
        assert!(p.required);
    }

    #[test]
    fn test_description_builder() {
        let p = ParamMeta::new("x").description("some help");
        assert_eq!(p.description, "some help");
    }

    #[test]
    fn test_short_builder() {
        let p = ParamMeta::new("x").short('v');
        assert_eq!(p.short, Some('v'));
    }

    #[test]
    fn test_aliases_builder() {
        static ALIASES: &[&str] = &["foo", "bar"];
        let p = ParamMeta::new("x").aliases(ALIASES);
        assert_eq!(p.aliases, &["foo", "bar"]);
    }

    #[test]
    fn test_param_type_builder() {
        let p = ParamMeta::new("x").param_type(ParamType::Integer);
        assert_eq!(p.param_type, ParamType::Integer);
    }

    #[test]
    fn test_short_opt_some() {
        let p = ParamMeta::new("x").short_opt(Some('z'));
        assert_eq!(p.short, Some('z'));
    }

    #[test]
    fn test_short_opt_none() {
        let p = ParamMeta::new("x").short('a').short_opt(None);
        assert!(p.short.is_none());
    }

    #[test]
    fn test_builder_chain() {
        static ALIASES: &[&str] = &["alt"];
        let p = ParamMeta::new("count")
            .required()
            .description("A count")
            .short('c')
            .aliases(ALIASES)
            .param_type(ParamType::Integer);
        assert_eq!(p.name, "count");
        assert!(p.required);
        assert_eq!(p.description, "A count");
        assert_eq!(p.short, Some('c'));
        assert_eq!(p.aliases, &["alt"]);
        assert_eq!(p.param_type, ParamType::Integer);
    }
}
