use std::path::{Path, PathBuf};
use tokio::task;

use super::cache::refresh_remote_repository_cache;
use super::models::RepositoryRecord;

#[derive(Clone)]
pub struct RepositoryStorage {
    pub local_root: PathBuf,
    pub remote_cache_root: PathBuf,
}

impl RepositoryStorage {
    pub fn new(local_root: PathBuf, remote_cache_root: PathBuf) -> Self {
        RepositoryStorage {
            local_root,
            remote_cache_root,
        }
    }

    pub fn ensure_local_repository(&self, segments: &[String]) -> anyhow::Result<PathBuf> {
        let mut path = self.local_root.clone();
        for segment in segments {
            path.push(segment);
        }

        // Repositories are stored with .git suffix (bare repositories)
        // Append .git to the final path to match the storage format
        let repo_path = path.with_extension("git");

        tracing::info!("ensure_local_repository query: {}", repo_path.display());

        // Check if this is a non-bare repository (has .git subdirectory)
        let git_dir = repo_path.join(".git");
        let actual_repo_path = if git_dir.is_dir() {
            tracing::info!("ensure_local_repository found non-bare repo, using .git subdirectory: {}", git_dir.display());
            git_dir
        } else if repo_path.is_dir() {
            tracing::info!("ensure_local_repository found bare repo: {}", repo_path.display());
            repo_path.clone()
        } else {
            tracing::info!("ensure_local_repository missing: {}", repo_path.display());
            return Err(anyhow::anyhow!(
                "repository directory not found at {}",
                repo_path.display()
            ));
        };

        Ok(actual_repo_path)
    }

    pub async fn ensure_remote_repository(
        &self,
        record: &RepositoryRecord,
    ) -> anyhow::Result<PathBuf> {
        let remote_url = record
            .remote_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("remote repository missing URL"))?;
        let cache_path = self.remote_cache_root.join(&record.id);
        let parent = cache_path.parent().map(Path::to_path_buf);

        let result = task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
            if let Some(parent) = &parent {
                std::fs::create_dir_all(parent).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to create remote cache directory {}: {}",
                        parent.display(),
                        err
                    )
                })?;
            }

            if cache_path.exists() {
                std::fs::remove_dir_all(&cache_path).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to reset remote cache at {}: {}",
                        cache_path.display(),
                        err
                    )
                })?;
            }

            let mut prepare =
                gix::prepare_clone(remote_url.clone(), &cache_path).map_err(|err| {
                    anyhow::anyhow!(
                        "failed to prepare clone for remote repository {}: {}",
                        remote_url,
                        err
                    )
                })?;
            let (repo, _) = prepare
                .fetch_only(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|err| {
                    anyhow::anyhow!("failed to clone remote repository {}: {}", remote_url, err)
                })?;

            refresh_remote_repository_cache(&repo, &remote_url)?;

            Ok(cache_path)
        })
        .await
        .map_err(|err| anyhow::anyhow!(err))??;

        Ok(result)
    }
}

impl git_http::RepositoryProvider for RepositoryStorage {
    fn ensure_local_repository(&self, segments: &[String]) -> anyhow::Result<PathBuf> {
        RepositoryStorage::ensure_local_repository(self, segments)
    }
}
