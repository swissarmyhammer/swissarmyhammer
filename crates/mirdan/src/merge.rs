//! Order-preserving, duplicate-free extension of a list.
//!
//! Several parts of Mirdan build a list by appending to it from more than one
//! source, and must not repeat a value the list already carries: `list`
//! combines the targets of packages found under the same name in several
//! places, the profile installer accumulates the targets each deployed item
//! landed on, and `status` builds the order in which it probes MCP config
//! keys. All three want the same merge, so all three call [`merge_unique`].

/// Append every item of `incoming` that `existing` does not already carry.
///
/// The order of `existing` is kept, and new items arrive in the order
/// `incoming` yields them. An item equal to one already present is dropped, so
/// merging the same source twice leaves `existing` unchanged.
pub fn merge_unique<T: PartialEq>(existing: &mut Vec<T>, incoming: impl IntoIterator<Item = T>) {
    for item in incoming {
        if !existing.contains(&item) {
            existing.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_unique_appends_items_the_list_does_not_carry() {
        let mut existing = vec!["Claude Code".to_string()];

        merge_unique(&mut existing, vec!["Cursor".to_string()]);

        assert_eq!(existing, vec!["Claude Code", "Cursor"]);
    }

    #[test]
    fn merge_unique_drops_an_item_the_list_already_carries() {
        let mut existing = vec!["Claude Code".to_string()];

        merge_unique(&mut existing, vec!["Claude Code".to_string()]);

        assert_eq!(existing, vec!["Claude Code"]);
    }

    #[test]
    fn merge_unique_keeps_the_order_of_both_sides() {
        let mut existing = vec!["project".to_string(), "global".to_string()];

        merge_unique(
            &mut existing,
            vec![
                "global".to_string(),
                "~/.validators".to_string(),
                "project".to_string(),
                "~/.tools".to_string(),
            ],
        );

        assert_eq!(
            existing,
            vec!["project", "global", "~/.validators", "~/.tools"]
        );
    }

    #[test]
    fn merge_unique_drops_a_duplicate_inside_incoming() {
        let mut existing: Vec<String> = Vec::new();

        merge_unique(
            &mut existing,
            vec!["Cursor".to_string(), "Cursor".to_string()],
        );

        assert_eq!(existing, vec!["Cursor"]);
    }

    #[test]
    fn merge_unique_accepts_an_array_of_borrowed_strings() {
        let mut existing: Vec<&str> = vec!["context_servers"];

        merge_unique(&mut existing, ["mcpServers", "servers", "context_servers"]);

        assert_eq!(existing, vec!["context_servers", "mcpServers", "servers"]);
    }

    #[test]
    fn merge_unique_leaves_the_list_alone_when_incoming_is_empty() {
        let mut existing = vec!["global".to_string()];

        merge_unique(&mut existing, Vec::new());

        assert_eq!(existing, vec!["global"]);
    }
}
