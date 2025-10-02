use std::collections::HashSet;
use std::path::PathBuf;

use sqlx::SqlitePool;
use tokio::task;

use super::db::resolve_repository_by_path;
use super::entries::{
    normalize_file_path, normalize_tree_path, read_repository_entries,
    read_repository_file_for_branch,
};
use super::models::{
    RepositoryBranch, RepositoryEntriesPayload, RepositoryFilePayload, RepositoryRecord,
    RepositorySummary, RepositorySummaryRow,
};
use super::storage::RepositoryStorage;
use crate::group::queries::get_group_parent;
use crate::validation::slug::validate_slug;

pub async fn get_all_repositories_raw(pool: &SqlitePool) -> anyhow::Result<Vec<RepositoryRecord>> {
    let records = sqlx::query_as::<_, RepositoryRecord>(
        "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories ORDER BY slug",
    )
    .fetch_all(pool)
    .await?;

    Ok(records)
}

pub async fn get_repository_by_id(
    pool: &SqlitePool,
    id: &str,
) -> anyhow::Result<Option<RepositoryRecord>> {
    let record = sqlx::query_as::<_, RepositoryRecord>(
        "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

pub async fn get_repository_raw(
    pool: &SqlitePool,
    path: String,
) -> anyhow::Result<Option<RepositoryRecord>> {
    resolve_repository_by_path(pool, &path)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn browse_repository_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    path: String,
    tree_path: Option<String>,
    branch: Option<String>,
) -> anyhow::Result<Option<RepositoryEntriesPayload>> {
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

    let record = resolve_repository_by_path(pool, &path).await?;
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
        read_repository_entries(repository_path, normalized_tree_path.clone(), branch).await?;

    Ok(Some(RepositoryEntriesPayload {
        tree_path: normalized_tree_path,
        entries,
    }))
}

pub async fn list_repository_branches_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    path: String,
) -> anyhow::Result<Option<Vec<RepositoryBranch>>> {
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

    let record = resolve_repository_by_path(pool, &path).await?;
    let Some(record) = record else {
        return Ok(None);
    };

    let repository_path = if record.remote_url.is_some() {
        storage.ensure_remote_repository(&record).await?
    } else {
        storage.ensure_local_repository(&segments)?
    };

    let branches = task::spawn_blocking(move || list_repository_branches_blocking(repository_path))
        .await
        .map_err(|err| anyhow::anyhow!(err))??;

    Ok(Some(branches))
}

fn list_repository_branches_blocking(
    repository_path: PathBuf,
) -> anyhow::Result<Vec<RepositoryBranch>> {
    let repo = gix::open(&repository_path).map_err(|err| {
        anyhow::anyhow!(
            "failed to open repository at {}: {}",
            repository_path.display(),
            err
        )
    })?;

    let head = repo.head().ok();
    let head_reference_name = head
        .as_ref()
        .and_then(|head| head.referent_name())
        .map(full_name_to_string);
    let head_commit = head
        .as_ref()
        .and_then(|head| head.id())
        .map(|id| id.to_string());

    let mut branches = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(mut iter) = repo.references()?.local_branches() {
        while let Some(result) = iter.next() {
            let reference = match result {
                Ok(reference) => reference,
                Err(_) => continue,
            };

            let full_name = full_name_to_string(reference.name());
            if !seen.insert(full_name.clone()) {
                continue;
            }

            let target = reference.try_id().map(|id| id.to_string());
            let is_default = head_reference_name
                .as_ref()
                .map(|name| name == &full_name)
                .unwrap_or_else(|| {
                    if let (Some(head_commit), Some(branch_commit)) = (&head_commit, &target) {
                        head_commit == branch_commit
                    } else {
                        false
                    }
                });

            branches.push(RepositoryBranch {
                name: short_branch_name(&full_name),
                reference: full_name,
                target,
                is_default,
            });
        }
    }

    if branches.is_empty() {
        let remote_head_target = repo
            .find_reference("refs/remotes/origin/HEAD")
            .ok()
            .and_then(|reference| match reference.target() {
                gix::refs::TargetRef::Symbolic(name) => Some(full_name_to_string(name)),
                _ => None,
            });

        if let Ok(mut iter) = repo.references()?.remote_branches() {
            while let Some(result) = iter.next() {
                let reference = match result {
                    Ok(reference) => reference,
                    Err(_) => continue,
                };

                let full_name = full_name_to_string(reference.name());
                if full_name.ends_with("/HEAD") {
                    continue;
                }
                if !seen.insert(full_name.clone()) {
                    continue;
                }

                let target = reference.try_id().map(|id| id.to_string());
                let is_default = remote_head_target
                    .as_ref()
                    .map(|name| name == &full_name)
                    .unwrap_or(false);

                branches.push(RepositoryBranch {
                    name: short_branch_name(&full_name),
                    reference: full_name,
                    target,
                    is_default,
                });
            }
        }
    }

    branches.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(branches)
}

fn full_name_to_string(name: &gix::refs::FullNameRef) -> String {
    use gix::bstr::ByteSlice;

    String::from_utf8_lossy(name.as_bstr().as_bytes()).into_owned()
}

fn short_branch_name(full_name: &str) -> String {
    if let Some(stripped) = full_name.strip_prefix("refs/heads/") {
        stripped.to_string()
    } else if let Some(stripped) = full_name.strip_prefix("refs/remotes/") {
        stripped.to_string()
    } else {
        full_name.to_string()
    }
}

pub async fn read_repository_file_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    path: String,
    file_path: String,
    branch: Option<String>,
) -> anyhow::Result<Option<RepositoryFilePayload>> {
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

    let record = resolve_repository_by_path(pool, &path).await?;
    let Some(record) = record else {
        return Ok(None);
    };

    let normalized_file_path = normalize_file_path(file_path)?;

    let repository_path = if record.remote_url.is_some() {
        storage.ensure_remote_repository(&record).await?
    } else {
        storage.ensure_local_repository(&segments)?
    };

    let file =
        read_repository_file_for_branch(repository_path, normalized_file_path, branch).await?;

    Ok(Some(file))
}

pub async fn get_repositories_for_group(
    pool: &SqlitePool,
    group_id: &str,
) -> anyhow::Result<Vec<RepositorySummary>> {
    let rows = sqlx::query_as::<_, RepositorySummaryRow>(
        "SELECT id, slug, remote_url FROM repositories WHERE \"group\" = ? ORDER BY slug",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(RepositorySummary::from).collect())
}

/// Reconstructs the full repository path from a repository record
async fn reconstruct_repository_path(
    pool: &SqlitePool,
    record: &RepositoryRecord,
) -> anyhow::Result<String> {
    if let Some(group_id) = &record.group_id {
        // Recursively build group path
        let mut path_segments = vec![record.slug.clone()];
        let mut current_group_id = group_id.clone();

        loop {
            if let Some(group) = get_group_parent(pool, &current_group_id).await? {
                path_segments.insert(0, group.slug.clone());
                if let Some(parent_id) = group.parent {
                    current_group_id = parent_id;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(path_segments.join("/"))
    } else {
        Ok(record.slug.clone())
    }
}

/// Gets the README HTML for a repository at the root path
pub async fn get_repository_readme_html(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    record: &RepositoryRecord,
    branch: Option<String>,
) -> anyhow::Result<Option<String>> {
    // Reconstruct full path
    let path = reconstruct_repository_path(pool, record).await?;

    let segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect();

    if segments.is_empty() {
        return Ok(None);
    }

    // Get repository entries at root
    let repository_path = if record.remote_url.is_some() {
        storage.ensure_remote_repository(record).await?
    } else {
        storage.ensure_local_repository(&segments)?
    };

    let entries = read_repository_entries(repository_path.clone(), String::new(), branch.clone()).await?;

    // Detect README file
    let readme_path = super::readme::detect_readme_file(&entries);
    let Some(readme_path) = readme_path else {
        return Ok(None);
    };

    // Read README content
    let file = read_repository_file_for_branch(
        repository_path,
        readme_path.clone(),
        branch.clone(),
    )
    .await?;

    // Skip binary files or empty content
    if file.is_binary || file.text.is_none() {
        return Ok(None);
    }

    let content = file.text.unwrap();
    let html = super::readme::render_readme(&content, &readme_path);

    Ok(Some(html))
}
