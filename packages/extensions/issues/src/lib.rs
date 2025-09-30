//! Issues Extension for Forge
//!
//! This extension adds issue tracking capabilities to the GraphQL API.
//! It demonstrates the extension interface implementation.

use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    world: "extension",
    path: "../../../packages/wit",
});

use exports::forge::extension::extension_api::{Config, ExtensionInfo, Guest, ResolveInfo, ResolveResult};

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
    fn init(config: Config) -> Result<(), String> {
        println!("Issues extension initialized: {}", config.name);
        Ok(())
    }

    fn get_info() -> ExtensionInfo {
        ExtensionInfo {
            name: "issues".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["basic".to_string(), "database".to_string()],
        }
    }

    fn get_schema() -> String {
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
  getAllIssues: [Issue!]!
  getIssue(id: ID!): Issue
}

extend type Mutation {
  createIssue(input: CreateIssueInput!): Issue!
  updateIssue(id: ID!, input: UpdateIssueInput!): Issue
}
"#.trim().to_string()
    }

    fn resolve_field(info: ResolveInfo) -> ResolveResult {
        match info.field_name.as_str() {
            "getAllIssues" => {
                let sample_issues = vec![Issue {
                    id: "issue-1".to_string(),
                    title: "Sample Issue".to_string(),
                    description: Some("This is a sample issue".to_string()),
                    status: "OPEN".to_string(),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                }];

                match serde_json::to_string(&sample_issues) {
                    Ok(json) => ResolveResult::Success(json),
                    Err(e) => ResolveResult::Error(format!("Serialization error: {}", e))
                }
            }
            "getIssue" => {
                ResolveResult::Success("null".to_string())
            }
            "createIssue" => {
                ResolveResult::Error("Not implemented".to_string())
            }
            "updateIssue" => {
                ResolveResult::Error("Not implemented".to_string())
            }
            _ => ResolveResult::Error(format!("Unknown field: {}", info.field_name))
        }
    }

    fn shutdown() {
        println!("Issues extension shutting down");
    }
}

export!(IssuesExtension);
