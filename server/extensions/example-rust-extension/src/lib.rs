//! Example Issues Extension for Forge
//!
//! This is a sample WASM extension that adds issue tracking capabilities
//! to the GraphQL API. It demonstrates the extension interface implementation.

use serde::{Deserialize, Serialize};

// Generate WIT bindings
wit_bindgen::generate!({
    world: "extension-host",
    path: "../../wit/extension.wit",
});

use exports::forge::extension::extension::{
    ApiInfo, EnumType, EnumValue, ExtensionConfig, FieldDefinition, Guest, InputObjectType,
    InputValueDefinition, ObjectType, SchemaFragment, SchemaType, TypeModifier, TypeRef,
};

#[derive(Serialize, Deserialize)]
struct Issue {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    created_at: String,
}

struct IssuesExtension;

impl Guest for IssuesExtension {
    fn get_api_info() -> ApiInfo {
        ApiInfo {
            version: "0.1.0".to_string(),
            supported_capabilities: vec!["basic".to_string(), "database".to_string()],
        }
    }

    fn init(config: ExtensionConfig) -> Result<(), String> {
        // Initialize the extension
        println!("Issues extension initialized: {}", config.name);

        // In a real implementation, we would:
        // - Set up database connection using config.db_path
        // - Initialize any required state
        // - Validate configuration

        Ok(())
    }

    fn get_schema() -> SchemaFragment {
        fn type_ref(root: &str, modifiers: &[TypeModifier]) -> TypeRef {
            TypeRef {
                root: root.to_string(),
                modifiers: modifiers.to_vec(),
            }
        }

        fn non_null(root: &str) -> TypeRef {
            type_ref(root, &[TypeModifier::NonNull])
        }

        fn list_of_non_null_items_non_null(root: &str) -> TypeRef {
            type_ref(
                root,
                &[
                    TypeModifier::NonNull,
                    TypeModifier::ListType,
                    TypeModifier::NonNull,
                ],
            )
        }

        SchemaFragment {
            types: vec![
                SchemaType::EnumType(EnumType {
                    name: "IssueStatus".to_string(),
                    description: Some("State of an issue".to_string()),
                    values: vec![
                        EnumValue {
                            name: "OPEN".to_string(),
                            description: Some("New issue awaiting work".to_string()),
                        },
                        EnumValue {
                            name: "CLOSED".to_string(),
                            description: Some("Issue resolved or dismissed".to_string()),
                        },
                        EnumValue {
                            name: "IN_PROGRESS".to_string(),
                            description: Some("Work is currently underway".to_string()),
                        },
                    ],
                }),
                SchemaType::ObjectType(ObjectType {
                    name: "Issue".to_string(),
                    description: Some("A tracked issue within Forge".to_string()),
                    interfaces: vec![],
                    fields: vec![
                        FieldDefinition {
                            name: "id".to_string(),
                            description: None,
                            ty: non_null("ID"),
                            args: vec![],
                        },
                        FieldDefinition {
                            name: "title".to_string(),
                            description: None,
                            ty: non_null("String"),
                            args: vec![],
                        },
                        FieldDefinition {
                            name: "description".to_string(),
                            description: None,
                            ty: type_ref("String", &[]),
                            args: vec![],
                        },
                        FieldDefinition {
                            name: "status".to_string(),
                            description: None,
                            ty: non_null("IssueStatus"),
                            args: vec![],
                        },
                        FieldDefinition {
                            name: "createdAt".to_string(),
                            description: None,
                            ty: non_null("String"),
                            args: vec![],
                        },
                    ],
                    is_extension: false,
                }),
                SchemaType::InputObjectType(InputObjectType {
                    name: "CreateIssueInput".to_string(),
                    description: Some("Input for creating issues".to_string()),
                    fields: vec![
                        InputValueDefinition {
                            name: "title".to_string(),
                            description: None,
                            ty: non_null("String"),
                            default_value: None,
                        },
                        InputValueDefinition {
                            name: "description".to_string(),
                            description: None,
                            ty: type_ref("String", &[]),
                            default_value: None,
                        },
                    ],
                }),
                SchemaType::InputObjectType(InputObjectType {
                    name: "UpdateIssueInput".to_string(),
                    description: Some("Input for updating issues".to_string()),
                    fields: vec![
                        InputValueDefinition {
                            name: "title".to_string(),
                            description: None,
                            ty: type_ref("String", &[]),
                            default_value: None,
                        },
                        InputValueDefinition {
                            name: "description".to_string(),
                            description: None,
                            ty: type_ref("String", &[]),
                            default_value: None,
                        },
                        InputValueDefinition {
                            name: "status".to_string(),
                            description: None,
                            ty: type_ref("IssueStatus", &[]),
                            default_value: None,
                        },
                    ],
                }),
                SchemaType::ObjectType(ObjectType {
                    name: "Query".to_string(),
                    description: None,
                    interfaces: vec![],
                    fields: vec![
                        FieldDefinition {
                            name: "getAllIssues".to_string(),
                            description: Some("Return all issues".to_string()),
                            ty: list_of_non_null_items_non_null("Issue"),
                            args: vec![],
                        },
                        FieldDefinition {
                            name: "getIssue".to_string(),
                            description: Some("Fetch a single issue".to_string()),
                            ty: type_ref("Issue", &[]),
                            args: vec![InputValueDefinition {
                                name: "id".to_string(),
                                description: None,
                                ty: non_null("ID"),
                                default_value: None,
                            }],
                        },
                    ],
                    is_extension: true,
                }),
                SchemaType::ObjectType(ObjectType {
                    name: "Mutation".to_string(),
                    description: None,
                    interfaces: vec![],
                    fields: vec![
                        FieldDefinition {
                            name: "createIssue".to_string(),
                            description: Some("Create a new issue".to_string()),
                            ty: non_null("Issue"),
                            args: vec![InputValueDefinition {
                                name: "input".to_string(),
                                description: None,
                                ty: non_null("CreateIssueInput"),
                                default_value: None,
                            }],
                        },
                        FieldDefinition {
                            name: "updateIssue".to_string(),
                            description: Some("Update an existing issue".to_string()),
                            ty: type_ref("Issue", &[]),
                            args: vec![
                                InputValueDefinition {
                                    name: "id".to_string(),
                                    description: None,
                                    ty: non_null("ID"),
                                    default_value: None,
                                },
                                InputValueDefinition {
                                    name: "input".to_string(),
                                    description: None,
                                    ty: non_null("UpdateIssueInput"),
                                    default_value: None,
                                },
                            ],
                        },
                    ],
                    is_extension: true,
                }),
            ],
        }
    }

    fn migrate(db_path: String) -> Result<(), String> {
        // Run database migrations
        println!("Running migrations for issues extension at: {}", db_path);

        // In a real implementation, we would:
        // - Open SQLite database at db_path
        // - Create tables if they don't exist
        // - Run any schema migrations
        // - Set up indexes

        Ok(())
    }

    fn resolve_field(field: String, _args: String) -> Result<String, String> {
        // Handle GraphQL field resolution
        match field.as_str() {
            "getAllIssues" => {
                // In a real implementation:
                // - Parse args JSON
                // - Query database
                // - Return serialized results

                let sample_issues = vec![Issue {
                    id: "issue-1".to_string(),
                    title: "Sample Issue".to_string(),
                    description: Some("This is a sample issue".to_string()),
                    status: "OPEN".to_string(),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                }];

                serde_json::to_string(&sample_issues)
                    .map_err(|e| format!("Serialization error: {}", e))
            }
            "getIssue" => {
                // Parse args to get issue ID
                // Query database for specific issue
                // Return issue or null

                Ok("null".to_string()) // No issue found
            }
            "createIssue" => {
                // Parse args to get CreateIssueInput
                // Validate input
                // Insert into database
                // Return created issue

                Err("Not implemented".to_string())
            }
            "updateIssue" => {
                // Parse args to get ID and UpdateIssueInput
                // Validate input
                // Update in database
                // Return updated issue

                Err("Not implemented".to_string())
            }
            _ => Err(format!("Unknown field: {}", field))
        }
    }
}

export!(IssuesExtension);
