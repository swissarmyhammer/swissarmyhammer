// Test fixture used by multi-turn tool-use integration tests.
// Content is deliberately small and unambiguous so assertions can match
// substrings ("main", "fn main", "hello") with high confidence after the
// model reads this file via the `read_file` MCP tool.

fn main() {
    println!("hello");
}
