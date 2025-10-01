//! Example Issues Extension for Forge
//!
//! This is a sample WASM extension that adds issue tracking capabilities
//! to the GraphQL API. It demonstrates the extension interface implementation.

use serde::{Deserialize, Serialize};

// Generate WIT bindings
wit_bindgen::generate!({
    world: "extension",
    path: "../../wit",
});

use exports::forge::extension::extension_api::{Config, ExtensionInfo, Guest, ResolveInfo, ResolveResult};

#[derive(Serialize, Deserialize)]
struct Issue {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "repositoryId")]
    repository_id: String,
}

struct IssuesExtension;

impl Guest for IssuesExtension {
    fn init(config: Config) -> Result<(), String> {
        // Initialize the extension
        println!("Issues extension initialized: {}", config.name);

        // In a real implementation, we would:
        // - Set up database connection using config.database_path
        // - Initialize any required state
        // - Validate configuration

        Ok(())
    }

    fn get_info() -> ExtensionInfo {
        ExtensionInfo {
            name: "issues".to_string(),
            version: "0.2.0".to_string(),
            capabilities: vec!["basic".to_string(), "database".to_string()],
        }
    }

    fn get_schema() -> String {
        // Return GraphQL SDL schema for the issues extension
        r#"
enum IssueStatus {
  OPEN
  CLOSED
  IN_PROGRESS
}

type Issue @key(fields: "id") {
  id: ID!
  title: String!
  description: String
  status: IssueStatus!
  createdAt: String!
  repositoryId: ID!
}

input CreateIssueInput {
  title: String!
  description: String
}

input UpdateIssueInput {
  title: String
  description: String
  status: IssueStatus
}

extend type Query {
  getIssuesForRepository(repositoryId: ID!): [Issue!]!
  getIssue(repositoryId: ID!, id: ID!): Issue
}

extend type Mutation {
  createIssue(repositoryId: ID!, input: CreateIssueInput!): Issue!
  updateIssue(repositoryId: ID!, id: ID!, input: UpdateIssueInput!): Issue
}
"#.trim().to_string()
    }

    fn resolve_field(info: ResolveInfo) -> ResolveResult {
        // Handle GraphQL field resolution
        match info.field_name.as_str() {
            "getIssuesForRepository" => {
                #[derive(Deserialize)]
                struct Args {
                    #[serde(rename = "repositoryId")]
                    repository_id: String,
                }

                let repository_id = serde_json::from_str::<Args>(&info.arguments)
                    .map(|args| args.repository_id)
                    .unwrap_or_else(|_| "repo-1".to_string());

                let sample_issues = vec![Issue {
                    id: "issue-1".to_string(),
                    title: "Sample Issue".to_string(),
                    description: Some("This is a sample issue".to_string()),
                    status: "OPEN".to_string(),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    repository_id,
                }];

                match serde_json::to_string(&sample_issues) {
                    Ok(json) => ResolveResult::Success(json),
                    Err(e) => ResolveResult::Error(format!("Serialization error: {}", e))
                }
            }
            "getIssue" => {
                // Parse args to get issue ID
                // Query database for specific issue
                // Return issue or null

                ResolveResult::Success("null".to_string()) // No issue found
            }
            "createIssue" => {
                // Parse args to get CreateIssueInput
                // Validate input
                // Insert into database
                // Return created issue

                ResolveResult::Error("Not implemented".to_string())
            }
            "updateIssue" => {
                // Parse args to get ID and UpdateIssueInput
                // Validate input
                // Update in database
                // Return updated issue

                ResolveResult::Error("Not implemented".to_string())
            }
            _ => ResolveResult::Error(format!("Unknown field: {}", info.field_name))
        }
    }

    fn shutdown() {
        // Clean shutdown
        println!("Issues extension shutting down");
    }
}

export!(IssuesExtension);
