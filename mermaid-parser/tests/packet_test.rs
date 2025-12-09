use mermaid_parser::parsers::packet;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_packet_files(#[files("test/packet/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let _diagram = packet::parse(&content).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });

    // Basic validation - packet diagrams should have fields
    // Note: some test files might be empty or incomplete, so we'll just ensure parsing succeeds
}

#[test]
fn test_simple_packet() {
    let input = r#"packet-beta
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields.len(), 3);

    let source_port = &diagram.fields[0];
    assert_eq!(source_port.start_bit, 0);
    assert_eq!(source_port.end_bit, 15);
    assert_eq!(source_port.name, "Source Port");
    assert!(!source_port.is_optional);
}

#[test]
fn test_single_bit_fields() {
    let input = r#"packet-beta
0: "Version"
1: "IHL"
2: "Type of Service"
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields.len(), 3);

    for field in &diagram.fields {
        assert_eq!(field.start_bit, field.end_bit);
    }
}

#[test]
fn test_optional_fields() {
    let input = r#"packet-beta
0-31: "Header"
32-63: (Options)
64-95: "Data"
"#;

    let diagram = packet::parse(input).unwrap();

    assert!(!diagram.fields[0].is_optional);
    assert!(diagram.fields[1].is_optional);
    assert!(!diagram.fields[2].is_optional);
}

#[test]
fn test_unquoted_identifiers() {
    let input = r#"packet-beta
0-7: Source
8-15: Destination
16-31: Payload
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields[0].name, "Source");
    assert_eq!(diagram.fields[1].name, "Destination");
    assert_eq!(diagram.fields[2].name, "Payload");
}

#[test]
fn test_tcp_header() {
    let input = r#"packet-beta
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
64-95: "Acknowledgment Number"
96-99: "Data Offset"
100-105: "Reserved"
106: "URG"
107: "ACK"
108: "PSH"
109: "RST"
110: "SYN"
111: "FIN"
112-127: "Window"
128-143: "Checksum"
144-159: "Urgent Pointer"
160-191: (Options and Padding)
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields.len(), 16);

    // Verify bit flags
    let urg = diagram.fields.iter().find(|f| f.name == "URG").unwrap();
    assert_eq!(urg.start_bit, 106);
    assert_eq!(urg.end_bit, 106);

    // Verify optional field
    let options = diagram
        .fields
        .iter()
        .find(|f| f.name == "Options and Padding")
        .unwrap();
    assert!(options.is_optional);
}

#[test]
fn test_basic_packet() {
    let input = r#"packet-beta
0-7: "Type"
8-15: "Length"
16-31: "Data"
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields.len(), 3);
    assert_eq!(diagram.fields[0].start_bit, 0);
    assert_eq!(diagram.fields[0].end_bit, 7);
    assert_eq!(diagram.fields[0].name, "Type");
    assert_eq!(diagram.fields[1].start_bit, 8);
    assert_eq!(diagram.fields[1].end_bit, 15);
    assert_eq!(diagram.fields[1].name, "Length");
    assert_eq!(diagram.fields[2].start_bit, 16);
    assert_eq!(diagram.fields[2].end_bit, 31);
    assert_eq!(diagram.fields[2].name, "Data");
}

#[test]
fn test_mixed_field_types() {
    let input = r#"packet-beta
0-7: "Header"
8: Flag1
9: Flag2
10-15: (Reserved)
16-31: "Payload"
"#;

    let diagram = packet::parse(input).unwrap();

    assert_eq!(diagram.fields.len(), 5);

    // Check quoted field
    assert_eq!(diagram.fields[0].name, "Header");
    assert!(!diagram.fields[0].is_optional);

    // Check unquoted fields
    assert_eq!(diagram.fields[1].name, "Flag1");
    assert_eq!(diagram.fields[2].name, "Flag2");

    // Check optional field
    assert_eq!(diagram.fields[3].name, "Reserved");
    assert!(diagram.fields[3].is_optional);

    // Check quoted field again
    assert_eq!(diagram.fields[4].name, "Payload");
    assert!(!diagram.fields[4].is_optional);
}
