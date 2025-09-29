use crate::graphql::errors::internal_error;

pub fn refresh_remote_repository_cache(
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
