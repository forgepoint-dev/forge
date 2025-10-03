//! Issues Extension for Forge
//!
//! This extension adds issue tracking capabilities to the GraphQL API with SQLite persistence.

#![allow(unsafe_op_in_unsafe_fn)]

use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

wit_bindgen::generate!({
    world: "extension",
    path: "../../../packages/wit/extension.wit",
});

use exports::forge::extension::extension_api::{
    Config, ContextScope, ExtensionInfo, Guest, ResolveInfo, ResolveResult,
};
use forge::extension::host_database::{self, RecordValue};
use forge::extension::host_log::{self, LogLevel};

const SCHEMA: &str = include_str!("../../shared/schema.graphql");

#[derive(Debug, Clone)]
struct Issue {
    db_id: String,
    repository_id: String,
    number: i64,
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
                repository_id TEXT NOT NULL,
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
                ensure_repository_schema().map_err(|e| format!("Migration failed: {}", e))
            }
            Err(e) => Err(format!("Migration failed: {}", e)),
        }
    }

    fn get_info() -> ExtensionInfo {
        ExtensionInfo {
            name: "issues".to_string(),
            version: "0.2.0".to_string(),
            capabilities: vec!["basic".to_string(), "database".to_string()],
        }
    }

    fn get_schema() -> String {
        SCHEMA.trim().to_string()
    }

    fn resolve_field(info: ResolveInfo) -> ResolveResult {
        let ResolveInfo {
            field_name,
            arguments,
            context,
            ..
        } = info;

        let scope = context.scope;
        let repository_context_id = context.repository.map(|ctx| ctx.id);

        if matches!(
            field_name.as_str(),
            "getIssuesForRepository" | "getIssue" | "createIssue" | "updateIssue"
        ) && !matches!(
            scope,
            ContextScope::Repository | ContextScope::RepositoryUser
        ) {
            return ResolveResult::Error(
                "Repository-scoped context required for issues operations".to_string(),
            );
        }

        match field_name.as_str() {
            "getIssuesForRepository" => {
                resolve_get_issues_for_repository(&arguments, repository_context_id.as_deref())
            }
            "getIssue" => resolve_get_issue(&arguments, repository_context_id.as_deref()),
            "createIssue" => resolve_create_issue(&arguments, repository_context_id.as_deref()),
            "updateIssue" => resolve_update_issue(&arguments, repository_context_id.as_deref()),
            _ => ResolveResult::Error(format!("Unknown field: {}", field_name)),
        }
    }

    fn shutdown() {
        host_log::log(LogLevel::Info, "Issues extension shutting down");
    }
}

fn resolve_get_issues_for_repository(
    arguments: &str,
    context_repository: Option<&str>,
) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        #[serde(rename = "repositoryId")]
        repository_id: String,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e)),
    };

    if let Err(err) = assert_repository_context(context_repository, &args.repository_id) {
        return ResolveResult::Error(err);
    }

    let sql = "SELECT id, repository_id, number, title, description, status, created_at FROM issues WHERE repository_id = ? ORDER BY number DESC";
    let params = vec![RecordValue::Text(args.repository_id.clone())];

    match host_database::query(sql, &params) {
        host_database::QueryResult::Success(rows) => {
            let issues: Vec<Issue> = rows
                .into_iter()
                .map(|row| issue_from_values(&row.values))
                .collect();
            serialize_issues(issues)
        }
        host_database::QueryResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn resolve_get_issue(arguments: &str, context_repository: Option<&str>) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        #[serde(rename = "repositoryId")]
        repository_id: String,
        #[serde(rename = "issueNumber")]
        issue_number: i64,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e)),
    };

    if let Err(err) = assert_repository_context(context_repository, &args.repository_id) {
        return ResolveResult::Error(err);
    }

    match query_issue_by_number(&args.repository_id, args.issue_number) {
        Ok(Some(issue)) => serialize_issue(issue),
        Ok(None) => ResolveResult::Success("null".to_string()),
        Err(err) => ResolveResult::Error(err),
    }
}

fn resolve_create_issue(arguments: &str, context_repository: Option<&str>) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        #[serde(rename = "repositoryId")]
        repository_id: String,
        input: CreateIssueInput,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e)),
    };

    if let Err(err) = assert_repository_context(context_repository, &args.repository_id) {
        return ResolveResult::Error(err);
    }

    let number = match next_issue_number(&args.repository_id) {
        Ok(num) => num,
        Err(err) => return ResolveResult::Error(err),
    };

    let db_id = format!("issue_{}_{}", chrono::Utc::now().timestamp_millis(), number);
    let created_at = chrono::Utc::now().to_rfc3339();

    let sql = "INSERT INTO issues (id, repository_id, number, title, description, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let params = vec![
        RecordValue::Text(db_id.clone()),
        RecordValue::Text(args.repository_id.clone()),
        RecordValue::Integer(number),
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
                db_id,
                repository_id: args.repository_id,
                number,
                title: args.input.title,
                description: args.input.description,
                status: "OPEN".to_string(),
                created_at,
            };
            serialize_issue(issue)
        }
        host_database::ExecResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn resolve_update_issue(arguments: &str, context_repository: Option<&str>) -> ResolveResult {
    #[derive(Deserialize)]
    struct Args {
        #[serde(rename = "repositoryId")]
        repository_id: String,
        #[serde(rename = "issueNumber")]
        issue_number: i64,
        input: UpdateIssueInput,
    }

    let args: Args = match serde_json::from_str(arguments) {
        Ok(a) => a,
        Err(e) => return ResolveResult::Error(format!("Invalid arguments: {}", e)),
    };

    if let Err(err) = assert_repository_context(context_repository, &args.repository_id) {
        return ResolveResult::Error(err);
    }

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

    let sql = format!(
        "UPDATE issues SET {} WHERE repository_id = ? AND number = ?",
        updates.join(", ")
    );
    params.push(RecordValue::Text(args.repository_id.clone()));
    params.push(RecordValue::Integer(args.issue_number));

    match host_database::execute(&sql, &params) {
        host_database::ExecResult::Success(info) => {
            if info.rows_affected == 0 {
                return ResolveResult::Success("null".to_string());
            }

            match query_issue_by_number(&args.repository_id, args.issue_number) {
                Ok(Some(issue)) => serialize_issue(issue),
                Ok(None) => ResolveResult::Success("null".to_string()),
                Err(err) => ResolveResult::Error(err),
            }
        }
        host_database::ExecResult::Error(e) => {
            ResolveResult::Error(format!("Database error: {}", e))
        }
    }
}

fn serialize_issues(issues: Vec<Issue>) -> ResolveResult {
    let payload: Vec<_> = issues.iter().map(issue_to_json).collect();
    match serde_json::to_string(&payload) {
        Ok(json) => ResolveResult::Success(json),
        Err(e) => ResolveResult::Error(format!("Serialization error: {}", e)),
    }
}

fn serialize_issue(issue: Issue) -> ResolveResult {
    let payload = issue_to_json(&issue);
    match serde_json::to_string(&payload) {
        Ok(json) => ResolveResult::Success(json),
        Err(e) => ResolveResult::Error(format!("Serialization error: {}", e)),
    }
}

fn assert_repository_context(
    context_repository: Option<&str>,
    repository_id: &str,
) -> Result<(), String> {
    if let Some(expected) = context_repository
        && expected != repository_id
    {
        return Err(format!(
            "Repository context mismatch: expected `{}`, got `{}`",
            expected, repository_id
        ));
    }
    Ok(())
}

fn issue_from_values(values: &[RecordValue]) -> Issue {
    Issue {
        db_id: extract_string(&values[0]),
        repository_id: extract_string(&values[1]),
        number: extract_integer(&values[2]),
        title: extract_string(&values[3]),
        description: extract_optional_string(&values[4]),
        status: extract_string(&values[5]),
        created_at: extract_string(&values[6]),
    }
}

fn issue_to_json(issue: &Issue) -> serde_json::Value {
    json!({
        "id": format!("{}:{}", issue.repository_id, issue.number),
        "number": issue.number,
        "title": issue.title,
        "description": issue.description,
        "status": issue.status,
        "createdAt": issue.created_at,
        "repositoryId": issue.repository_id,
    })
}

fn query_issue_by_number(repository_id: &str, number: i64) -> Result<Option<Issue>, String> {
    let sql = "SELECT id, repository_id, number, title, description, status, created_at FROM issues WHERE repository_id = ? AND number = ?";
    let params = vec![
        RecordValue::Text(repository_id.to_string()),
        RecordValue::Integer(number),
    ];

    match host_database::query(sql, &params) {
        host_database::QueryResult::Success(rows) => Ok(rows
            .into_iter()
            .next()
            .map(|row| issue_from_values(&row.values))),
        host_database::QueryResult::Error(e) => Err(format!("Database error: {}", e)),
    }
}

fn next_issue_number(repository_id: &str) -> Result<i64, String> {
    let sql = "SELECT COALESCE(MAX(number), 0) FROM issues WHERE repository_id = ?";
    let params = vec![RecordValue::Text(repository_id.to_string())];

    match host_database::query(sql, &params) {
        host_database::QueryResult::Success(rows) => {
            let current = rows
                .get(0)
                .and_then(|row| row.values.get(0))
                .map(extract_integer)
                .unwrap_or(0);
            Ok(current + 1)
        }
        host_database::QueryResult::Error(e) => {
            Err(format!("Failed to determine next issue number: {}", e))
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

fn extract_integer(value: &RecordValue) -> i64 {
    match value {
        RecordValue::Integer(i) => *i,
        RecordValue::Text(text) => text.parse::<i64>().unwrap_or(0),
        RecordValue::Null => 0,
        _ => 0,
    }
}

fn ensure_repository_schema() -> Result<(), String> {
    let columns = match host_database::query("PRAGMA table_info(issues);", &[]) {
        host_database::QueryResult::Success(rows) => rows,
        host_database::QueryResult::Error(e) => {
            return Err(format!("Failed to inspect issues table: {}", e));
        }
    };

    let has_repository_column = columns.iter().any(|row| {
        row.values
            .get(1)
            .and_then(|value| match value {
                RecordValue::Text(name) => Some(name == "repository_id"),
                _ => None,
            })
            .unwrap_or(false)
    });

    if !has_repository_column {
        host_log::log(
            LogLevel::Info,
            "Migrating issues table to add repository_id column",
        );
        match host_database::execute(
            "ALTER TABLE issues ADD COLUMN repository_id TEXT DEFAULT 'legacy'",
            &[],
        ) {
            host_database::ExecResult::Success(_) => {
                // ensure rows receive a non-null value
                let _ = host_database::execute(
                    "UPDATE issues SET repository_id = 'legacy' WHERE repository_id IS NULL",
                    &[],
                );
            }
            host_database::ExecResult::Error(e) => {
                return Err(format!(
                    "Failed to add repository_id column to issues table: {}",
                    e
                ));
            }
        }
    }

    let has_number_column = columns.iter().any(|row| {
        row.values
            .get(1)
            .and_then(|value| match value {
                RecordValue::Text(name) => Some(name == "number"),
                _ => None,
            })
            .unwrap_or(false)
    });

    if !has_number_column {
        host_log::log(
            LogLevel::Info,
            "Migrating issues table to add number column",
        );
        match host_database::execute("ALTER TABLE issues ADD COLUMN number INTEGER", &[]) {
            host_database::ExecResult::Success(_) => {
                backfill_issue_numbers()?;
            }
            host_database::ExecResult::Error(e) => {
                return Err(format!(
                    "Failed to add number column to issues table: {}",
                    e
                ));
            }
        }
    }

    ensure_index(
        "CREATE INDEX IF NOT EXISTS idx_issues_repository ON issues(repository_id, created_at DESC)",
        "repository index",
    )?;
    ensure_index(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_issues_repository_number ON issues(repository_id, number)",
        "repository number index",
    )?;

    Ok(())
}

fn ensure_index(statement: &str, label: &str) -> Result<(), String> {
    match host_database::execute(statement, &[]) {
        host_database::ExecResult::Success(_) => Ok(()),
        host_database::ExecResult::Error(e) => Err(format!("Failed to ensure {}: {}", label, e)),
    }
}

fn backfill_issue_numbers() -> Result<(), String> {
    let rows = match host_database::query(
        "SELECT id, repository_id, created_at FROM issues ORDER BY repository_id, created_at",
        &[],
    ) {
        host_database::QueryResult::Success(rows) => rows,
        host_database::QueryResult::Error(e) => {
            return Err(format!("Failed to read issues for backfill: {}", e));
        }
    };

    let mut counters: HashMap<String, i64> = HashMap::new();

    for row in rows {
        let id = extract_string(&row.values[0]);
        let repository_id = extract_string(&row.values[1]);
        let counter = counters.entry(repository_id.clone()).or_insert(0);
        *counter += 1;

        let params = vec![RecordValue::Integer(*counter), RecordValue::Text(id)];
        match host_database::execute("UPDATE issues SET number = ? WHERE id = ?", &params) {
            host_database::ExecResult::Success(_) => {}
            host_database::ExecResult::Error(e) => {
                return Err(format!("Failed to backfill issue number: {}", e));
            }
        }
    }

    Ok(())
}

export!(IssuesExtension);
