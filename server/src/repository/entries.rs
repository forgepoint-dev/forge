use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::task;

use super::models::{RepositoryEntryKind, RepositoryEntryNode};
use crate::graphql::errors::{bad_user_input, internal_error};

pub fn normalize_tree_path(tree_path: Option<String>) -> async_graphql::Result<String> {
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

pub async fn read_repository_entries(
    repository_path: PathBuf,
    tree_path: String,
) -> async_graphql::Result<Vec<RepositoryEntryNode>> {
    task::spawn_blocking(move || list_repository_entries(&repository_path, &tree_path))
        .await
        .map_err(internal_error)?
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
        .map_err(internal_error)?;
    let root_tree = commit.tree().map_err(internal_error)?;

    let tree = if tree_path.is_empty() {
        root_tree
    } else {
        let entry = root_tree
            .lookup_entry_by_path(Path::new(tree_path))
            .map_err(internal_error)?
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
            .map_err(internal_error)?
            .into_tree()
    };

    let mut entries = Vec::new();

    for entry in tree.iter() {
        let entry = entry.map_err(internal_error)?;
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
                    .map_err(internal_error)?
                    .into_blob();
                entries.push(RepositoryEntryNode {
                    name,
                    path: full_path,
                    kind: RepositoryEntryKind::File,
                    size: Some(blob.data.len() as i64),
                });
            }
            gix::object::tree::EntryKind::Commit => {
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
