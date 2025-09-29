mod db;

use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql::{
    Context, EmptySubscription, Error, ErrorExtensions, ID, InputObject, Object, Schema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::Router;
use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::response::Html;
use axum::routing::{get, post};
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

#[derive(Clone, Debug, sqlx::FromRow)]
struct GroupRecord {
    id: String,
    slug: String,
    parent: Option<String>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct RepositoryRecord {
    id: String,
    slug: String,
    group_id: Option<String>,
    remote_url: Option<String>,
}

#[derive(Clone)]
struct GroupNode(GroupRecord);

#[derive(Clone)]
struct GroupSummary {
    id: String,
    slug: String,
}

#[derive(Clone)]
struct RepositoryNode(RepositoryRecord);

#[derive(Clone)]
struct RepositorySummary {
    id: String,
    slug: String,
    remote_url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct RepositorySummaryRow {
    id: String,
    slug: String,
    remote_url: Option<String>,
}

#[Object]
impl GroupNode {
    async fn id(&self) -> ID {
        ID::from(self.0.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.0.slug
    }

    async fn parent(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<GroupSummary>> {
        let Some(ref parent_id) = self.0.parent else {
            return Ok(None);
        };

        let pool = ctx.data::<SqlitePool>()?;
        let parent = fetch_group_by_id(pool, parent_id)
            .await
            .map_err(internal_error)?;
        Ok(parent.map(GroupSummary::from))
    }

    async fn repositories(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<RepositorySummary>> {
        let pool = ctx.data::<SqlitePool>()?;
        let rows = sqlx::query_as::<_, RepositorySummaryRow>(
            "SELECT id, slug, remote_url FROM repositories WHERE \"group\" = ? ORDER BY slug",
        )
        .bind(&self.0.id)
        .fetch_all(pool)
        .await
        .map_err(internal_error)?;

        Ok(rows.into_iter().map(RepositorySummary::from).collect())
    }
}

#[Object]
impl GroupSummary {
    async fn id(&self) -> ID {
        ID::from(self.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.slug
    }
}

impl From<GroupRecord> for GroupNode {
    fn from(record: GroupRecord) -> Self {
        GroupNode(record)
    }
}

impl From<GroupRecord> for GroupSummary {
    fn from(record: GroupRecord) -> Self {
        GroupSummary {
            id: record.id,
            slug: record.slug,
        }
    }
}

#[Object]
impl RepositoryNode {
    async fn id(&self) -> ID {
        ID::from(self.0.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.0.slug
    }

    async fn group(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<GroupSummary>> {
        let Some(ref group_id) = self.0.group_id else {
            return Ok(None);
        };

        let pool = ctx.data::<SqlitePool>()?;
        let group = fetch_group_by_id(pool, group_id)
            .await
            .map_err(internal_error)?;
        Ok(group.map(GroupSummary::from))
    }

    #[graphql(name = "isRemote")]
    async fn is_remote(&self) -> bool {
        self.0.remote_url.is_some()
    }

    #[graphql(name = "remoteUrl")]
    async fn remote_url(&self) -> Option<&str> {
        self.0.remote_url.as_deref()
    }
}

#[Object]
impl RepositorySummary {
    async fn id(&self) -> ID {
        ID::from(self.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.slug
    }

    #[graphql(name = "isRemote")]
    async fn is_remote(&self) -> bool {
        self.remote_url.is_some()
    }

    #[graphql(name = "remoteUrl")]
    async fn remote_url(&self) -> Option<&str> {
        self.remote_url.as_deref()
    }
}

impl From<RepositoryRecord> for RepositoryNode {
    fn from(record: RepositoryRecord) -> Self {
        RepositoryNode(record)
    }
}

impl From<RepositorySummaryRow> for RepositorySummary {
    fn from(row: RepositorySummaryRow) -> Self {
        RepositorySummary {
            id: row.id,
            slug: row.slug,
            remote_url: row.remote_url,
        }
    }
}

#[derive(Default)]
struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn get_all_groups(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<GroupNode>> {
        let pool = ctx.data::<SqlitePool>()?;
        let records =
            sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups ORDER BY slug")
                .fetch_all(pool)
                .await
                .map_err(internal_error)?;

        Ok(records.into_iter().map(GroupNode::from).collect())
    }

    async fn get_all_repositories(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<RepositoryNode>> {
        let pool = ctx.data::<SqlitePool>()?;
        let records = sqlx::query_as::<_, RepositoryRecord>(
            "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories ORDER BY slug",
        )
        .fetch_all(pool)
        .await
        .map_err(internal_error)?;

        Ok(records.into_iter().map(RepositoryNode::from).collect())
    }

    async fn get_group(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<GroupNode>> {
        let pool = ctx.data::<SqlitePool>()?;
        let record = resolve_group_by_path(pool, &path)
            .await
            .map_err(internal_error)?;
        Ok(record.map(GroupNode::from))
    }

    async fn get_repository(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<RepositoryNode>> {
        let pool = ctx.data::<SqlitePool>()?;
        let record = resolve_repository_by_path(pool, &path)
            .await
            .map_err(internal_error)?;
        Ok(record.map(RepositoryNode::from))
    }
}

#[derive(InputObject)]
struct CreateGroupInput {
    slug: String,
    #[graphql(name = "parent")]
    parent: Option<ID>,
}

#[derive(InputObject)]
struct CreateRepositoryInput {
    slug: String,
    #[graphql(name = "group")]
    group: Option<ID>,
}

#[derive(Default)]
struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_group(
        &self,
        ctx: &Context<'_>,
        input: CreateGroupInput,
    ) -> async_graphql::Result<GroupNode> {
        validate_slug(&input.slug)?;

        let pool = ctx.data::<SqlitePool>()?;
        let parent_id = match input.parent {
            Some(id) => {
                let id = id.to_string();
                let exists = fetch_group_by_id(pool, &id)
                    .await
                    .map_err(internal_error)?
                    .is_some();
                if !exists {
                    return Err(bad_user_input("parent group not found"));
                }
                Some(id)
            }
            None => None,
        };

        if slug_conflicts_for_group(pool, parent_id.as_deref(), &input.slug)
            .await
            .map_err(internal_error)?
        {
            return Err(bad_user_input("slug already exists in this group"));
        }

        let id = cuid2::create_id();
        sqlx::query("INSERT INTO groups (id, slug, parent) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&input.slug)
            .bind(parent_id.as_ref())
            .execute(pool)
            .await
            .map_err(internal_error)?;

        let record = GroupRecord {
            id,
            slug: input.slug,
            parent: parent_id,
        };

        Ok(GroupNode::from(record))
    }

    async fn create_repository(
        &self,
        ctx: &Context<'_>,
        input: CreateRepositoryInput,
    ) -> async_graphql::Result<RepositoryNode> {
        validate_slug(&input.slug)?;

        let pool = ctx.data::<SqlitePool>()?;
        let group_id = match input.group {
            Some(id) => {
                let id = id.to_string();
                let exists = fetch_group_by_id(pool, &id)
                    .await
                    .map_err(internal_error)?
                    .is_some();
                if !exists {
                    return Err(bad_user_input("group not found"));
                }
                Some(id)
            }
            None => None,
        };

        if slug_conflicts_for_repository(pool, group_id.as_deref(), &input.slug)
            .await
            .map_err(internal_error)?
        {
            return Err(bad_user_input("slug already exists in this group"));
        }

        let id = cuid2::create_id();
        sqlx::query(
            "INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, ?, ?) ",
        )
        .bind(&id)
        .bind(&input.slug)
        .bind(group_id.as_ref())
        .bind::<Option<&str>>(None)
        .execute(pool)
        .await
        .map_err(internal_error)?;

        let record = RepositoryRecord {
            id,
            slug: input.slug,
            group_id,
            remote_url: None,
        };

        Ok(RepositoryNode::from(record))
    }

    async fn link_remote_repository(
        &self,
        ctx: &Context<'_>,
        url: String,
    ) -> async_graphql::Result<RepositoryNode> {
        let pool = ctx.data::<SqlitePool>()?;

        let (normalized_url, slug) = normalize_remote_repository(&url)?;

        if remote_url_exists(pool, &normalized_url)
            .await
            .map_err(internal_error)?
        {
            return Err(bad_user_input("remote repository already linked"));
        }

        validate_slug(&slug)?;

        if slug_conflicts_for_repository(pool, None, &slug)
            .await
            .map_err(internal_error)?
        {
            return Err(bad_user_input("slug already exists at the root"));
        }

        let id = cuid2::create_id();
        sqlx::query(
            "INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, NULL, ?)",
        )
        .bind(&id)
        .bind(&slug)
        .bind(&normalized_url)
        .execute(pool)
        .await
        .map_err(internal_error)?;

        let record = RepositoryRecord {
            id,
            slug,
            group_id: None,
            remote_url: Some(normalized_url),
        };

        Ok(RepositoryNode::from(record))
    }
}

type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

async fn graphql_handler(State(schema): State<AppSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground() -> Html<String> {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"),
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool: SqlitePool = db::init_pool().await?;

    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(pool)
    .finish();

    let app = Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", post(graphql_handler).options(graphql_options))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods([Method::POST, Method::OPTIONS]),
        )
        .with_state(schema);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn bad_user_input(message: impl Into<String>) -> Error {
    Error::new(message.into()).extend_with(|_, e| e.set("code", "BAD_USER_INPUT"))
}

async fn graphql_options() -> StatusCode {
    StatusCode::NO_CONTENT
}

fn validate_slug(slug: &str) -> async_graphql::Result<()> {
    let is_valid = !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    if is_valid {
        Ok(())
    } else {
        Err(bad_user_input("slug must be lowercase kebab-case"))
    }
}

fn internal_error(err: impl std::fmt::Display) -> Error {
    Error::new(err.to_string())
}

async fn fetch_group_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

async fn slug_conflicts_for_group(
    pool: &SqlitePool,
    parent_id: Option<&str>,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    if let Some(parent_id) = parent_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM groups WHERE slug = ? AND parent = ? LIMIT 1")
                .bind(slug)
                .bind(parent_id)
                .fetch_optional(pool)
                .await?;
        Ok(exists.is_some())
    } else {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM groups WHERE slug = ? AND parent IS NULL LIMIT 1")
                .bind(slug)
                .fetch_optional(pool)
                .await?;
        Ok(exists.is_some())
    }
}

async fn slug_conflicts_for_repository(
    pool: &SqlitePool,
    group_id: Option<&str>,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    if let Some(group_id) = group_id {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM repositories WHERE slug = ? AND \"group\" = ? LIMIT 1",
        )
        .bind(slug)
        .bind(group_id)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
    } else {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM repositories WHERE slug = ? AND \"group\" IS NULL LIMIT 1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
    }
}

async fn resolve_group_by_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Ok(None);
    }

    let mut parent_id: Option<String> = None;
    let mut current: Option<GroupRecord> = None;

    for slug in segments {
        let record = if let Some(ref parent) = parent_id {
            sqlx::query_as::<_, GroupRecord>(
                "SELECT id, slug, parent FROM groups WHERE slug = ? AND parent = ?",
            )
            .bind(slug)
            .bind(parent)
            .fetch_optional(pool)
            .await?
        } else {
            sqlx::query_as::<_, GroupRecord>(
                "SELECT id, slug, parent FROM groups WHERE slug = ? AND parent IS NULL",
            )
            .bind(slug)
            .fetch_optional(pool)
            .await?
        };

        match record {
            Some(row) => {
                parent_id = Some(row.id.clone());
                current = Some(row);
            }
            None => return Ok(None),
        }
    }

    Ok(current)
}

async fn resolve_repository_by_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<RepositoryRecord>, sqlx::Error> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Ok(None);
    }

    let (group_segments, repo_part) = segments.split_at(segments.len() - 1);
    let repo_slug = repo_part[0];

    let group_id = if group_segments.is_empty() {
        None
    } else {
        let group_path = group_segments.join("/");
        match resolve_group_by_path(pool, &group_path).await? {
            Some(group) => Some(group.id),
            None => return Ok(None),
        }
    };

    match group_id.as_deref() {
        Some(group_id) => sqlx::query_as::<_, RepositoryRecord>(
            "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" = ?",
        )
        .bind(repo_slug)
        .bind(group_id)
        .fetch_optional(pool)
        .await,
        None => sqlx::query_as::<_, RepositoryRecord>(
            "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" IS NULL",
        )
        .bind(repo_slug)
        .fetch_optional(pool)
        .await,
    }
}

async fn remote_url_exists(pool: &SqlitePool, remote_url: &str) -> Result<bool, sqlx::Error> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM repositories WHERE remote_url = ? LIMIT 1")
            .bind(remote_url)
            .fetch_optional(pool)
            .await?;

    Ok(exists.is_some())
}

fn normalize_remote_repository(raw_url: &str) -> async_graphql::Result<(String, String)> {
    let mut url =
        Url::parse(raw_url).map_err(|_| bad_user_input("invalid remote repository URL"))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(bad_user_input("only http(s) remote URLs are supported")),
    }

    url.set_fragment(None);
    url.set_query(None);

    let slug = slug_from_remote_url(&url)?;

    let mut normalized: String = url.into();
    while normalized.ends_with('/') {
        normalized.pop();
    }

    Ok((normalized, slug))
}

fn slug_from_remote_url(url: &Url) -> async_graphql::Result<String> {
    let segments: Vec<_> = url
        .path_segments()
        .ok_or_else(|| bad_user_input("remote URL is missing path segments"))?
        .filter(|segment| !segment.is_empty())
        .collect();

    let Some(last_segment) = segments.last() else {
        return Err(bad_user_input("remote URL must include a repository name"));
    };

    let candidate = last_segment.trim_end_matches(".git").to_ascii_lowercase();

    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in candidate.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            slug.push(ch);
            last_was_dash = false;
        } else if ch == '-' {
            if !last_was_dash && !slug.is_empty() {
                slug.push('-');
                last_was_dash = true;
            }
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();

    if slug.is_empty() {
        return Err(bad_user_input(
            "repository name in URL cannot be converted to a valid slug",
        ));
    }

    Ok(slug)
}
