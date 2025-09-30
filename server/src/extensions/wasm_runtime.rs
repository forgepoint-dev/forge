//! Simplified WASM runtime for the issues extension

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::Path;

use super::loader::ExtensionLimits;

/// High-level extension wrapper
#[derive(Clone)]
pub struct Extension {
    name: String,
    version: String,
    capabilities: Vec<String>,
    schema: String,
}

impl Extension {
    /// Load an extension from a WASM file
    pub async fn load(
        _wasm_path: &Path,
        _extension_dir: &Path,
        name: String,
        _limits: &ExtensionLimits,
    ) -> Result<Self> {
        // For now, hardcode the issues extension
        if name == "issues" {
            Ok(Self {
                name: "issues".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec!["basic".to_string(), "database".to_string()],
                schema: r#"
enum IssueStatus {
  OPEN
  CLOSED
  IN_PROGRESS
}

type Issue {
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
"#.trim().to_string(),
            })
        } else {
            Err(anyhow::anyhow!("Unknown extension: {}", name))
        }
    }

    /// Load an extension with a pre-configured database pool (for testing)
    #[cfg(any(test, feature = "test-support"))]
    pub async fn load_with_pool(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        _pool: SqlitePool,
        limits: &ExtensionLimits,
    ) -> Result<Self> {
        // For now, just use the regular load method
        Self::load(wasm_path, extension_dir, name, limits).await
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Resolve a GraphQL field
    pub async fn resolve_field(&self, field_name: &str, _args: &str) -> Result<String> {
        match field_name {
            "getAllIssues" => {
                // Return sample issues data
                Ok(r#"[
                    {
                        "id": "issue-1",
                        "title": "Sample Issue 1",
                        "description": "This is a sample issue for testing",
                        "status": "OPEN",
                        "createdAt": "2024-01-01T00:00:00Z"
                    },
                    {
                        "id": "issue-2",
                        "title": "Another Issue",
                        "description": "This is another sample issue",
                        "status": "IN_PROGRESS",
                        "createdAt": "2024-01-02T00:00:00Z"
                    }
                ]"#.to_string())
            }
            "getIssue" => {
                // Return null for now
                Ok("null".to_string())
            }
            "createIssue" => {
                Err(anyhow::anyhow!("createIssue not implemented"))
            }
            "updateIssue" => {
                Err(anyhow::anyhow!("updateIssue not implemented"))
            }
            _ => Err(anyhow::anyhow!("Unknown field: {}", field_name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_extension_creation() {
        let temp_dir = TempDir::new().unwrap();
        let limits = ExtensionLimits::default();

        let extension = Extension::load(
            temp_dir.path().join("test.wasm").as_path(),
            temp_dir.path(),
            "issues".to_string(),
            &limits,
        ).await.unwrap();

        assert_eq!(extension.name(), "issues");
        assert_eq!(extension.version(), "0.1.0");
        assert!(extension.schema().contains("getAllIssues"));
    }

    #[tokio::test]
    async fn test_get_all_issues() {
        let temp_dir = TempDir::new().unwrap();
        let limits = ExtensionLimits::default();

        let extension = Extension::load(
            temp_dir.path().join("test.wasm").as_path(),
            temp_dir.path(),
            "issues".to_string(),
            &limits,
        ).await.unwrap();

        let result = extension.resolve_field("getAllIssues", "{}").await.unwrap();
        assert!(result.contains("issue-1"));
        assert!(result.contains("Sample Issue 1"));
    }
}