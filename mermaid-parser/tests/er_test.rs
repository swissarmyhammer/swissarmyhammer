use mermaid_parser::parse_diagram;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_er_files(#[files("test/er/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty files
    if content.is_empty() {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Er(_diagram) => {
            // Just verify it parsed successfully - some test files might be empty
        }
        _ => panic!("Expected ER diagram from {:?}", path),
    }
}

#[test]
fn test_simple_er_diagram() {
    let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name PK
        string customerId PK
        string address
        string phoneNumber
    }
    ORDER ||--|{ LINE-ITEM : contains
    ORDER {
        int orderId PK
        string customerId FK
        date orderDate
        string status
    }
    LINE-ITEM {
        string productId FK
        int quantity
        float pricePerUnit
    }
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Er(diagram) => {
            assert_eq!(diagram.entities.len(), 3);
            assert_eq!(diagram.relationships.len(), 2);

            // Check CUSTOMER entity
            let customer = &diagram.entities["CUSTOMER"];
            assert_eq!(customer.attributes.len(), 4);
            assert_eq!(customer.attributes[0].name, "name");
            assert_eq!(
                customer.attributes[0].key_type,
                Some(mermaid_parser::KeyType::PK)
            );

            // Check ORDER entity
            let order = &diagram.entities["ORDER"];
            assert_eq!(order.attributes.len(), 4);
            assert_eq!(
                order.attributes[1].key_type,
                Some(mermaid_parser::KeyType::FK)
            );

            // Check relationships
            assert_eq!(diagram.relationships[0].left_entity, "CUSTOMER");
            assert_eq!(diagram.relationships[0].right_entity, "ORDER");
            assert_eq!(diagram.relationships[0].label, Some("places".to_string()));
        }
        _ => panic!("Expected ER diagram"),
    }
}

#[test]
fn test_attribute_types() {
    let input = r#"erDiagram
    PRODUCT {
        string productId PK
        string name
        float price
        boolean inStock
        date lastUpdated
        text description "Product description"
    }
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Er(diagram) => {
            let product = &diagram.entities["PRODUCT"];
            assert_eq!(product.attributes.len(), 6);

            let price_attr = product
                .attributes
                .iter()
                .find(|a| a.name == "price")
                .unwrap();
            assert_eq!(price_attr.attr_type, "float");

            let desc_attr = product
                .attributes
                .iter()
                .find(|a| a.name == "description")
                .unwrap();
            assert_eq!(desc_attr.comment, Some("Product description".to_string()));
        }
        _ => panic!("Expected ER diagram"),
    }
}

#[test]
fn test_cardinality_types() {
    let input = r#"erDiagram
    A ||--|| B : "one-to-one"
    C ||--o{ D : "one-to-many"
    E }o--|| F : "many-to-one"
    G }o--o{ H : "many-to-many"
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Er(diagram) => {
            assert_eq!(diagram.relationships.len(), 4);

            // Verify different cardinality patterns are parsed
            let one_to_one = &diagram.relationships[0];
            assert_eq!(
                one_to_one.left_cardinality.min,
                mermaid_parser::CardinalityValue::One
            );
            assert_eq!(
                one_to_one.left_cardinality.max,
                mermaid_parser::CardinalityValue::One
            );
            assert_eq!(
                one_to_one.right_cardinality.min,
                mermaid_parser::CardinalityValue::One
            );
            assert_eq!(
                one_to_one.right_cardinality.max,
                mermaid_parser::CardinalityValue::One
            );

            let one_to_many = &diagram.relationships[1];
            assert_eq!(
                one_to_many.left_cardinality.min,
                mermaid_parser::CardinalityValue::One
            );
            assert_eq!(
                one_to_many.left_cardinality.max,
                mermaid_parser::CardinalityValue::One
            );
            assert_eq!(
                one_to_many.right_cardinality.min,
                mermaid_parser::CardinalityValue::Zero
            );
            assert_eq!(
                one_to_many.right_cardinality.max,
                mermaid_parser::CardinalityValue::Many
            );
        }
        _ => panic!("Expected ER diagram"),
    }
}
