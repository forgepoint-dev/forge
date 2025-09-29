use super::*;
use crate::extensions::schema::{
    enum_type::{EnumType, EnumValue},
    field::FieldDefinition,
    fragment::{InputObjectType, SchemaFragment, SchemaType, UnionType},
    interface::InterfaceType,
    object::ObjectType,
    scalar::ScalarType,
    types::{InputValueDefinition, TypeModifier, TypeRef},
};

#[test]
fn test_ensure_unique_field_names_success() {
    let fields = vec![
        FieldDefinition {
            name: "field1".to_string(),
            description: None,
            ty: TypeRef {
                root: "String".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
        FieldDefinition {
            name: "field2".to_string(),
            description: None,
            ty: TypeRef {
                root: "Int".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
    ];

    let result = ensure_unique_field_names(&fields, "test_extension", "TestObject");
    assert!(result.is_ok());
}

#[test]
fn test_ensure_unique_field_names_duplicate() {
    let fields = vec![
        FieldDefinition {
            name: "duplicate".to_string(),
            description: None,
            ty: TypeRef {
                root: "String".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
        FieldDefinition {
            name: "duplicate".to_string(),
            description: None,
            ty: TypeRef {
                root: "Int".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
    ];

    let result = ensure_unique_field_names(&fields, "test_extension", "TestObject");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Duplicate field name"));
}

#[test]
fn test_ensure_unique_names_success() {
    let names = vec!["value1", "value2", "value3"];
    let result = ensure_unique_names(
        names.into_iter(),
        "test_extension",
        "TestEnum",
        "enum value",
    );
    assert!(result.is_ok());
}

#[test]
fn test_ensure_unique_names_duplicate() {
    let names = vec!["value1", "value2", "value1"];
    let result = ensure_unique_names(
        names.into_iter(),
        "test_extension",
        "TestEnum",
        "enum value",
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Duplicate"));
}

#[test]
fn test_is_root_type() {
    assert!(is_root_type("Query"));
    assert!(is_root_type("Mutation"));
    assert!(is_root_type("Subscription"));
    assert!(!is_root_type("User"));
    assert!(!is_root_type("query")); // case sensitive
    assert!(!is_root_type(""));
}

#[test]
fn test_schema_fragment_empty() {
    let fragment = SchemaFragment::default();
    assert!(fragment.is_empty());
    assert!(fragment.types.is_empty());

    let sdl = fragment.to_sdl();
    assert_eq!(sdl, "");
}

#[test]
fn test_schema_fragment_with_object_type() {
    let mut fragment = SchemaFragment::default();
    fragment.types.push(SchemaType::Object(ObjectType {
        description: Some("Test object type".to_string()),
        fields: vec![
            FieldDefinition {
                name: "id".to_string(),
                description: Some("Object ID".to_string()),
                ty: TypeRef {
                    root: "ID".to_string(),
                    modifiers: vec![],
                },
                args: vec![],
            },
            FieldDefinition {
                name: "name".to_string(),
                description: None,
                ty: TypeRef {
                    root: "String".to_string(),
                    modifiers: vec![],
                },
                args: vec![],
            },
        ],
        interfaces: vec![],
    }));

    assert!(!fragment.is_empty());
}

#[test]
fn test_type_ref_to_sdl() {
    // Simple scalar
    let scalar = TypeRef {
        root: "String".to_string(),
        modifiers: vec![],
    };
    assert_eq!(scalar.to_sdl(), "String");

    // NonNull scalar
    let non_null = TypeRef {
        root: "Int".to_string(),
        modifiers: vec![TypeModifier::NonNull],
    };
    assert_eq!(non_null.to_sdl(), "Int!");

    // List
    let list = TypeRef {
        root: "Boolean".to_string(),
        modifiers: vec![TypeModifier::ListType],
    };
    assert_eq!(list.to_sdl(), "[Boolean]");

    // NonNull List of NonNull
    let complex = TypeRef {
        root: "ID".to_string(),
        modifiers: vec![TypeModifier::NonNull, TypeModifier::ListType, TypeModifier::NonNull],
    };
    assert_eq!(complex.to_sdl(), "[ID!]!");
}

#[test]
fn test_field_with_arguments() {
    let field = FieldDefinition {
        name: "search".to_string(),
        description: Some("Search for items".to_string()),
        ty: TypeRef {
            root: "SearchResult".to_string(),
            modifiers: vec![TypeModifier::ListType],
        },
        args: vec![
            InputValueDefinition {
                name: "query".to_string(),
                description: None,
                ty: TypeRef {
                    root: "String".to_string(),
                    modifiers: vec![TypeModifier::NonNull],
                },
                default_value: None,
            },
            InputValueDefinition {
                name: "limit".to_string(),
                description: None,
                ty: TypeRef {
                    root: "Int".to_string(),
                    modifiers: vec![],
                },
                default_value: Some("10".to_string()),
            },
        ],
    };

    let sdl = field.to_sdl(0);
    assert!(sdl.contains("search"));
    assert!(sdl.contains("query: String!"));
    assert!(sdl.contains("limit: Int"));
    assert!(sdl.contains("[SearchResult]"));
}

#[test]
fn test_multiple_types_in_fragment() {
    let mut fragment = SchemaFragment::default();

    fragment.types.push(SchemaType::Scalar(ScalarType {
        description: None,
    }));

    fragment.types.push(SchemaType::Enum(EnumType {
        description: None,
        values: vec![
            EnumValue {
                name: "ACTIVE".to_string(),
                description: None,
            },
            EnumValue {
                name: "INACTIVE".to_string(),
                description: None,
            },
        ],
    }));

    fragment.types.push(SchemaType::Object(ObjectType {
        description: None,
        fields: vec![
            FieldDefinition {
                name: "status".to_string(),
                description: None,
                ty: TypeRef {
                    root: "Status".to_string(),
                    modifiers: vec![],
                },
                args: vec![],
            },
        ],
        interfaces: vec![],
    }));

    let sdl = fragment.to_sdl();
    // Should have multiple types separated by newlines
    assert!(sdl.contains("\n\n"));
}

#[test]
fn test_empty_enum_validation() {
    let values: Vec<&str> = vec![];
    let result = ensure_unique_names(
        values.into_iter(),
        "test_extension",
        "EmptyEnum",
        "enum value",
    );
    assert!(result.is_ok()); // Empty is technically valid from uniqueness perspective
}

#[test]
fn test_case_sensitive_duplicates() {
    let fields = vec![
        FieldDefinition {
            name: "field".to_string(),
            description: None,
            ty: TypeRef {
                root: "String".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
        FieldDefinition {
            name: "Field".to_string(), // Different case
            description: None,
            ty: TypeRef {
                root: "String".to_string(),
                modifiers: vec![],
            },
            args: vec![],
        },
    ];

    let result = ensure_unique_field_names(&fields, "test_extension", "TestObject");
    assert!(result.is_ok()); // Case-sensitive, so these are different fields
}

#[test]
fn test_input_object_type() {
    let input_obj = InputObjectType {
        description: Some("Test input".to_string()),
        fields: vec![
            InputValueDefinition {
                name: "required".to_string(),
                description: None,
                ty: TypeRef {
                    root: "String".to_string(),
                    modifiers: vec![TypeModifier::NonNull],
                },
                default_value: None,
            },
            InputValueDefinition {
                name: "optional".to_string(),
                description: Some("Optional field".to_string()),
                ty: TypeRef {
                    root: "Int".to_string(),
                    modifiers: vec![],
                },
                default_value: Some("42".to_string()),
            },
        ],
    };

    assert_eq!(input_obj.fields.len(), 2);
    assert_eq!(input_obj.fields[0].name, "required");
    assert_eq!(input_obj.fields[1].default_value, Some("42".to_string()));
}

#[test]
fn test_union_type() {
    let union = UnionType {
        description: Some("A union of types".to_string()),
        types: vec!["TypeA".to_string(), "TypeB".to_string(), "TypeC".to_string()],
    };

    assert_eq!(union.types.len(), 3);
    assert!(union.types.contains(&"TypeA".to_string()));
    assert!(union.types.contains(&"TypeB".to_string()));
    assert!(union.types.contains(&"TypeC".to_string()));
}

#[test]
fn test_object_with_interfaces() {
    let object = ObjectType {
        description: None,
        fields: vec![
            FieldDefinition {
                name: "id".to_string(),
                description: None,
                ty: TypeRef {
                    root: "ID".to_string(),
                    modifiers: vec![TypeModifier::NonNull],
                },
                args: vec![],
            },
        ],
        interfaces: vec!["Node".to_string(), "Entity".to_string()],
    };

    assert_eq!(object.interfaces.len(), 2);
    assert!(object.interfaces.contains(&"Node".to_string()));
    assert!(object.interfaces.contains(&"Entity".to_string()));
}

#[test]
fn test_interface_type() {
    let interface = InterfaceType {
        description: Some("Base interface".to_string()),
        fields: vec![
            FieldDefinition {
                name: "id".to_string(),
                description: None,
                ty: TypeRef {
                    root: "ID".to_string(),
                    modifiers: vec![TypeModifier::NonNull],
                },
                args: vec![],
            },
        ],
    };

    assert_eq!(interface.fields.len(), 1);
    assert_eq!(interface.fields[0].name, "id");
}