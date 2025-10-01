//! Issues Extension for Forge
//!
//! This extension adds issue tracking capabilities to the GraphQL API with SQLite persistence.

use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    world: "extension",
    path: "../../../packages/wit",
});

use exports::forge::extension::extension_api::{Config, ExtensionInfo, Guest, ResolveInfo, ResolveResult};
use forge::extension::host_database::{self, RecordValue};
use forge::extension::host_log::{self, LogLevel};

#[derive(Serialize, Deserialize, Debug)]
struct Issue {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Deserialize)]
struct CreateIssueInput {
    title: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct UpdateIssueInput {
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
}

struct IssuesExtension;

impl Guest for IssuesExtension {
    fn init(_config: Config) -> Result<(), String> {
        let migrations = r#"
            CREATE TABLE IF NOT EXISTS issues (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'OPEN',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
            CREATE INDEX IF NOT EXISTS idx_issues_created_at ON issues(created_at);
        "#;

        match host_database::migrate(migrations) {
            Ok(_) => {
                host_log::log(LogLevel::Info, "Issues extension initialized with database");
                Ok(())
            }
            Err(e) => Err(format!("Migration failed: {}", e))
        }
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
            "getAllIssues" => resolve_get_all_issues(),
            "getIssue" => resolve_get_issue(&info.arguments),
            "createIssue" => resolve_create_issue(&info.arguments),
            "updateIssue" => resolve_update_issue(&info.arguments),
            _ => ResolveResult::Error(format!("Unknown field: {}", info.field_name))
        }
    }

    fn shutdown() {
        host_log::log(LogLevel::Info, "Issues extension shutting down");
    }
}

fn resolve_get_all_issues() -> ResolveResult {
    let sql = "SELECT id, title, description, status, created_at FROM issues ORDER BY created_at DESC";

    match host_database::query(sql, &[]) {
        host_database::QueryResult::Success(rows) => {
            let issues: Vec<Issue> = rows.into_iter().map(|row| {
                let values = row.values;
                Issue {
                    id: extract_string(&values[0]),
                    title: extract_string(&values[1]),
                    description: extract_optional_string(&values[2]),
                    status: extract_string(&values[3]),
                    created_at: extract_string(&values[4]),
                }
            }).collect();

            match serde_json::to_string(&issues) {
                Ok(json) => ResolveResult::Success(json),
                Err(e) => ResolveResult::Error(format!("Serialization error: {}", e))
            }
        }
        host_database::QueryResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn resolve_get_issue(arguments: &str) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        id: String,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e))
    };

    let sql = "SELECT id, title, description, status, created_at FROM issues WHERE id = ?";
    let params = vec![RecordValue::Text(args.id)];

    match host_database::query(sql, &params) {
        host_database::QueryResult::Success(rows) => {
            if rows.is_empty() {
                return ResolveResult::Success("null".to_string());
            }

            let row = &rows[0];
            let issue = Issue {
                id: extract_string(&row.values[0]),
                title: extract_string(&row.values[1]),
                description: extract_optional_string(&row.values[2]),
                status: extract_string(&row.values[3]),
                created_at: extract_string(&row.values[4]),
            };

            match serde_json::to_string(&issue) {
                Ok(json) => ResolveResult::Success(json),
                Err(e) => ResolveResult::Error(format!("Serialization error: {}", e))
            }
        }
        host_database::QueryResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn resolve_create_issue(arguments: &str) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        input: CreateIssueInput,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e))
    };

    let id = format!("issue_{}", chrono::Utc::now().timestamp());
    let created_at = chrono::Utc::now().to_rfc3339();

    let sql = "INSERT INTO issues (id, title, description, status, created_at) VALUES (?, ?, ?, ?, ?)";
    let params = vec![
        RecordValue::Text(id.clone()),
        RecordValue::Text(args.input.title.clone()),
        match &args.input.description {
            Some(d) => RecordValue::Text(d.clone()),
            None => RecordValue::Null,
        },
        RecordValue::Text("OPEN".to_string()),
        RecordValue::Text(created_at.clone()),
    ];

    match host_database::execute(sql, &params) {
        host_database::ExecResult::Success(_) => {
            let issue = Issue {
                id,
                title: args.input.title,
                description: args.input.description,
                status: "OPEN".to_string(),
                created_at,
            };

            match serde_json::to_string(&issue) {
                Ok(json) => ResolveResult::Success(json),
                Err(e) => ResolveResult::Error(format!("Serialization error: {}", e))
            }
        }
        host_database::ExecResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn resolve_update_issue(arguments: &str) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        id: String,
        input: UpdateIssueInput,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e))
    };

    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let Some(title) = args.input.title {
        updates.push("title = ?");
        params.push(RecordValue::Text(title));
    }
    if let Some(description) = args.input.description {
        updates.push("description = ?");
        params.push(RecordValue::Text(description));
    }
    if let Some(status) = args.input.status {
        updates.push("status = ?");
        params.push(RecordValue::Text(status));
    }

    if updates.is_empty() {
        return ResolveResult::Error("No fields to update".to_string());
    }

    let sql = format!("UPDATE issues SET {} WHERE id = ?", updates.join(", "));
    params.push(RecordValue::Text(args.id.clone()));

    match host_database::execute(&sql, &params) {
        host_database::ExecResult::Success(info) => {
            if info.rows_affected == 0 {
                return ResolveResult::Success("null".to_string());
            }

            resolve_get_issue(&format!(r#"{{"id":"{}"}}"#, args.id))
        }
        host_database::ExecResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn extract_string(value: &RecordValue) -> String {
    match value {
        RecordValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn extract_optional_string(value: &RecordValue) -> Option<String> {
    match value {
        RecordValue::Text(s) => Some(s.clone()),
        RecordValue::Null => None,
        _ => None,
    }
}

export!(IssuesExtension);
