use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::task;

use super::models::{RepositoryEntryKind, RepositoryEntryNode, RepositoryFilePayload};

const MAX_FILE_PREVIEW_BYTES: usize = 128 * 1024;

pub fn normalize_tree_path(tree_path: Option<String>) -> anyhow::Result<String> {
    let Some(tree_path) = tree_path else {
        return Ok(String::new());
    };

    let mut segments = Vec::new();
    for segment in tree_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return Err(anyhow::anyhow!("treePath cannot traverse upwards"));
        }

        if segment.contains('\0') {
            return Err(anyhow::anyhow!("treePath contains an invalid character"));
        }

        segments.push(segment.to_string());
    }

    Ok(segments.join("/"))
}

pub fn normalize_file_path(file_path: String) -> anyhow::Result<String> {
    if file_path.trim().is_empty() {
        return Err(anyhow::anyhow!("filePath cannot be empty"));
    }

    let mut segments = Vec::new();
    for segment in file_path.split('/') {
        if segment.is_empty() || segment == "." {
            return Err(anyhow::anyhow!("filePath contains an empty segment"));
        }

        if segment == ".." {
            return Err(anyhow::anyhow!("filePath cannot traverse upwards"));
        }

        if segment.contains('\0') {
            return Err(anyhow::anyhow!("filePath contains an invalid character"));
        }

        segments.push(segment.to_string());
    }

    if segments.is_empty() {
        return Err(anyhow::anyhow!("filePath must reference a file"));
    }

    Ok(segments.join("/"))
}

pub async fn read_repository_entries(
    repository_path: PathBuf,
    tree_path: String,
) -> anyhow::Result<Vec<RepositoryEntryNode>> {
    task::spawn_blocking(move || list_repository_entries(&repository_path, &tree_path))
        .await
        .map_err(|err| anyhow::anyhow!(err))?
}

pub async fn read_repository_file(
    repository_path: PathBuf,
    file_path: String,
) -> anyhow::Result<RepositoryFilePayload> {
    task::spawn_blocking(move || read_repository_file_blocking(repository_path, file_path))
        .await
        .map_err(|err| anyhow::anyhow!(err))?
}

fn read_repository_file_blocking(
    repository_path: PathBuf,
    file_path: String,
) -> anyhow::Result<RepositoryFilePayload> {
    let repo = gix::open(&repository_path).map_err(|err| {
        anyhow::anyhow!(
            "failed to open repository at {}: {}",
            repository_path.display(),
            err
        )
    })?;

    let mut head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            return Err(anyhow::anyhow!("repository does not contain any commits"));
        }
    };

    let commit = head.peel_to_commit_in_place()?;
    let root_tree = commit.tree()?;

    let entry = root_tree
        .lookup_entry_by_path(Path::new(&file_path))?
        .ok_or_else(|| anyhow::anyhow!("path `{}` not found in repository", file_path))?;

    match entry.mode().kind() {
        gix::object::tree::EntryKind::Blob
        | gix::object::tree::EntryKind::BlobExecutable
        | gix::object::tree::EntryKind::Link => {
            let blob = repo.find_object(entry.oid())?.into_blob();
            let data = &blob.data;

            let size = data.len() as i64;
            let truncated = data.len() > MAX_FILE_PREVIEW_BYTES;
            let preview_slice: &[u8] = if truncated {
                &data[..MAX_FILE_PREVIEW_BYTES]
            } else {
                data.as_slice()
            };

            let is_binary = std::str::from_utf8(data.as_slice()).is_err();
            let text = if is_binary {
                None
            } else {
                Some(std::str::from_utf8(preview_slice)?.to_string())
            };

            let name = Path::new(&file_path)
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("failed to resolve file name for `{}`", file_path))?
                .to_string();

            Ok(RepositoryFilePayload {
                path: file_path,
                name,
                size,
                is_binary,
                text,
                truncated,
            })
        }
        _ => Err(anyhow::anyhow!("path `{}` is not a file", file_path)),
    }
}

fn list_repository_entries(
    repository_path: &Path,
    tree_path: &str,
) -> anyhow::Result<Vec<RepositoryEntryNode>> {
    let repo = gix::open(repository_path).map_err(|err| {
        anyhow::anyhow!(
            "failed to open repository at {}: {}",
            repository_path.display(),
            err
        )
    })?;

    let mut head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            if tree_path.is_empty() {
                return Ok(Vec::new());
            }

            return Err(anyhow::anyhow!(
                "path `{}` not found in repository",
                tree_path
            ));
        }
    };

    let commit = head
        .peel_to_commit_in_place()
        .map_err(|err| anyhow::anyhow!(err))?;
    let root_tree = commit.tree().map_err(|err| anyhow::anyhow!(err))?;

    let tree = if tree_path.is_empty() {
        root_tree
    } else {
        let entry = root_tree
            .lookup_entry_by_path(Path::new(tree_path))
            .map_err(|err| anyhow::anyhow!(err))?
            .ok_or_else(|| anyhow::anyhow!("path `{}` not found in repository", tree_path))?;

        if !entry.mode().is_tree() {
            return Err(anyhow::anyhow!("path `{}` is not a directory", tree_path));
        }

        entry
            .object()
            .map_err(|err| anyhow::anyhow!(err))?
            .into_tree()
    };

    let mut entries = Vec::new();

    for entry in tree.iter() {
        let entry = entry.map_err(|err| anyhow::anyhow!(err))?;
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
                    .map_err(|err| anyhow::anyhow!(err))?
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
