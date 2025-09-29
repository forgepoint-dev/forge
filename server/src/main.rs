mod db;
mod supervisor;

use anyhow::Context as _;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql::{
    Context, EmptySubscription, Enum, Error, ErrorExtensions, ID, InputObject, Object, Schema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::Router;
use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::response::Html;
use axum::routing::{get, post};
use sqlx::SqlitePool;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use supervisor::Supervisor;
use tokio::task;
use tokio_util::sync::CancellationToken;
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

#[derive(Clone)]
struct RepositoryStorage {
    local_root: PathBuf,
    remote_cache_root: PathBuf,
}

#[derive(Clone)]
struct RepositoryEntriesPayload {
    tree_path: String,
    entries: Vec<RepositoryEntryNode>,
}

#[derive(Clone, PartialEq, Eq)]
struct RepositoryEntryNode {
    name: String,
    path: String,
    kind: RepositoryEntryKind,
    size: Option<i64>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum RepositoryEntryKind {
    #[graphql(name = "FILE")]
    File,
    #[graphql(name = "DIRECTORY")]
    Directory,
}

#[Object]
impl RepositoryEntriesPayload {
    async fn tree_path(&self) -> &str {
        &self.tree_path
    }

    async fn entries(&self) -> &[RepositoryEntryNode] {
        &self.entries
    }
}

#[Object]
impl RepositoryEntryNode {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn path(&self) -> &str {
        &self.path
    }

    async fn kind(&self) -> RepositoryEntryKind {
        self.kind
    }

    async fn size(&self) -> Option<i64> {
        self.size
    }
}

impl RepositoryStorage {
    fn new(local_root: PathBuf, remote_cache_root: PathBuf) -> Self {
        RepositoryStorage {
            local_root,
            remote_cache_root,
        }
    }

    fn ensure_local_repository(&self, segments: &[String]) -> async_graphql::Result<PathBuf> {
        let mut path = self.local_root.clone();
        for segment in segments {
            path.push(segment);
        }

        if path.is_dir() {
            Ok(path)
        } else {
            Err(internal_error(format!(
                "repository directory not found at {}",
                path.display()
            )))
        }
    }

    async fn ensure_remote_repository(
        &self,
        record: &RepositoryRecord,
    ) -> async_graphql::Result<PathBuf> {
        let remote_url = record
            .remote_url
            .clone()
            .ok_or_else(|| internal_error("remote repository missing URL"))?;
        let cache_path = self.remote_cache_root.join(&record.id);
        let parent = cache_path.parent().map(Path::to_path_buf);

        let result = task::spawn_blocking(move || -> async_graphql::Result<PathBuf> {
            if let Some(parent) = &parent {
                std::fs::create_dir_all(parent).map_err(|err| {
                    internal_error(format!(
                        "failed to create remote cache directory {}: {}",
                        parent.display(),
                        err
                    ))
                })?;
            }

            if cache_path.exists() {
                std::fs::remove_dir_all(&cache_path).map_err(|err| {
                    internal_error(format!(
                        "failed to reset remote cache at {}: {}",
                        cache_path.display(),
                        err
                    ))
                })?;
            }

            let mut prepare =
                gix::prepare_clone(remote_url.clone(), &cache_path).map_err(|err| {
                    internal_error(format!(
                        "failed to prepare clone for remote repository {}: {}",
                        remote_url, err
                    ))
                })?;
            let (repo, _) = prepare
                .fetch_only(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|err| {
                    internal_error(format!(
                        "failed to clone remote repository {}: {}",
                        remote_url, err
                    ))
                })?;

            refresh_remote_repository_cache(&repo, &remote_url)?;

            Ok(cache_path)
        })
        .await
        .map_err(|err| internal_error(err))??;

        Ok(result)
    }
}

fn refresh_remote_repository_cache(
    repo: &gix::Repository,
    remote_url: &str,
) -> async_graphql::Result<()> {
    repo.find_reference("refs/remotes/origin/HEAD")
        .map_err(|err| {
            internal_error(format!(
                "failed to resolve remote HEAD for {}: {}",
                remote_url, err
            ))
        })?;

    Ok(())
}

fn normalize_tree_path(tree_path: Option<String>) -> async_graphql::Result<String> {
    let Some(tree_path) = tree_path else {
        return Ok(String::new());
    };

    let mut segments = Vec::new();
    for segment in tree_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return Err(bad_user_input("treePath cannot traverse upwards"));
        }

        if segment.contains('\0') {
            return Err(bad_user_input("treePath contains an invalid character"));
        }

        segments.push(segment.to_string());
    }

    Ok(segments.join("/"))
}

async fn read_repository_entries(
    repository_path: PathBuf,
    tree_path: String,
) -> async_graphql::Result<Vec<RepositoryEntryNode>> {
    task::spawn_blocking(move || list_repository_entries(&repository_path, &tree_path))
        .await
        .map_err(|err| internal_error(err))?
}

fn list_repository_entries(
    repository_path: &Path,
    tree_path: &str,
) -> async_graphql::Result<Vec<RepositoryEntryNode>> {
    let repo = gix::open(repository_path).map_err(|err| {
        internal_error(format!(
            "failed to open repository at {}: {}",
            repository_path.display(),
            err
        ))
    })?;

    let mut head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            if tree_path.is_empty() {
                return Ok(Vec::new());
            }

            return Err(bad_user_input(format!(
                "path `{}` not found in repository",
                tree_path
            )));
        }
    };

    let commit = head
        .peel_to_commit_in_place()
        .map_err(|err| internal_error(err))?;
    let root_tree = commit.tree().map_err(|err| internal_error(err))?;

    let tree = if tree_path.is_empty() {
        root_tree
    } else {
        let entry = root_tree
            .lookup_entry_by_path(Path::new(tree_path))
            .map_err(|err| internal_error(err))?
            .ok_or_else(|| {
                bad_user_input(format!("path `{}` not found in repository", tree_path))
            })?;

        if !entry.mode().is_tree() {
            return Err(bad_user_input(format!(
                "path `{}` is not a directory",
                tree_path
            )));
        }

        entry
            .object()
            .map_err(|err| internal_error(err))?
            .into_tree()
    };

    let mut entries = Vec::new();

    for entry in tree.iter() {
        let entry = entry.map_err(|err| internal_error(err))?;
        let name = entry.filename().to_string();

        let full_path = if tree_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", tree_path, name)
        };

        match entry.mode().kind() {
            gix::object::tree::EntryKind::Tree => entries.push(RepositoryEntryNode {
                name,
                path: full_path,
                kind: RepositoryEntryKind::Directory,
                size: None,
            }),
            gix::object::tree::EntryKind::Blob
            | gix::object::tree::EntryKind::BlobExecutable
            | gix::object::tree::EntryKind::Link => {
                let blob = repo
                    .find_object(entry.oid())
                    .map_err(|err| internal_error(err))?
                    .into_blob();
                entries.push(RepositoryEntryNode {
                    name,
                    path: full_path,
                    kind: RepositoryEntryKind::File,
                    size: Some(blob.data.len() as i64),
                });
            }
            gix::object::tree::EntryKind::Commit => {
                // Submodules don't expose size; treat as directory for navigation purposes.
                entries.push(RepositoryEntryNode {
                    name,
                    path: full_path,
                    kind: RepositoryEntryKind::Directory,
                    size: None,
                });
            }
        }
    }

    entries.sort_by(|a, b| match (a.kind, b.kind) {
        (RepositoryEntryKind::Directory, RepositoryEntryKind::File) => Ordering::Less,
        (RepositoryEntryKind::File, RepositoryEntryKind::Directory) => Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });

    Ok(entries)
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

    async fn browse_repository(
        &self,
        ctx: &Context<'_>,
        path: String,
        #[graphql(name = "treePath")] tree_path: Option<String>,
    ) -> async_graphql::Result<Option<RepositoryEntriesPayload>> {
        let pool = ctx.data::<SqlitePool>()?;
        let storage = ctx.data::<RepositoryStorage>()?;

        let segments: Vec<String> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect();

        if segments.is_empty() {
            return Ok(None);
        }

        for segment in &segments {
            validate_slug(segment)?;
        }

        let record = resolve_repository_by_path(pool, &path)
            .await
            .map_err(internal_error)?;

        let Some(record) = record else {
            return Ok(None);
        };

        let normalized_tree_path = normalize_tree_path(tree_path)?;

        let repository_path = if record.remote_url.is_some() {
            storage.ensure_remote_repository(&record).await?
        } else {
            storage.ensure_local_repository(&segments)?
        };

        let entries =
            read_repository_entries(repository_path, normalized_tree_path.clone()).await?;

        Ok(Some(RepositoryEntriesPayload {
            tree_path: normalized_tree_path,
            entries,
        }))
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
    let (pool, db_root_path) = db::init_pool().await?;

    let repos_root_raw = std::env::var("FORGE_REPOS_PATH")
        .with_context(|| "FORGE_REPOS_PATH environment variable must be set".to_string())?;
    let repos_root = db::normalize_path(repos_root_raw)?;

    std::fs::create_dir_all(&repos_root).with_context(|| {
        format!(
            "failed to create repository root directory: {}",
            repos_root.display()
        )
    })?;

    let remote_cache_root = db_root_path.join("remote-cache");
    std::fs::create_dir_all(&remote_cache_root).with_context(|| {
        format!(
            "failed to create remote cache directory: {}",
            remote_cache_root.display()
        )
    })?;

    let storage = RepositoryStorage::new(repos_root, remote_cache_root);

    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(pool.clone())
    .data(storage.clone())
    .finish();

    let mut supervisor = Supervisor::new();

    supervisor.spawn("api", move |shutdown| async move {
        run_api(schema, shutdown).await
    });

    supervisor.run().await
}

fn bad_user_input(message: impl Into<String>) -> Error {
    Error::new(message.into()).extend_with(|_, e| e.set("code", "BAD_USER_INPUT"))
}

fn build_api_router(schema: AppSchema) -> Router {
    Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", post(graphql_handler).options(graphql_options))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods([Method::POST, Method::OPTIONS]),
        )
        .with_state(schema)
}

async fn run_api(schema: AppSchema, shutdown: CancellationToken) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, build_api_router(schema))
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
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
            .await
        } else {
            sqlx::query_as::<_, GroupRecord>(
                "SELECT id, slug, parent FROM groups WHERE slug = ? AND parent IS NULL",
            )
            .bind(slug)
            .fetch_optional(pool)
            .await
        }?;

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
        Some(group_id) => {
            sqlx::query_as::<_, RepositoryRecord>(
                "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" = ?",
            )
            .bind(repo_slug)
            .bind(group_id)
            .fetch_optional(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, RepositoryRecord>(
                "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" IS NULL",
            )
            .bind(repo_slug)
            .fetch_optional(pool)
            .await
        }
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
        "http" | "https" => {} // OK
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
