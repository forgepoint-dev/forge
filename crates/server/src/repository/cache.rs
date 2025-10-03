pub fn refresh_remote_repository_cache(
    repo: &gix::Repository,
    remote_url: &str,
) -> anyhow::Result<()> {
    repo.find_reference("refs/remotes/origin/HEAD")
        .map_err(|err| {
            anyhow::anyhow!("failed to resolve remote HEAD for {}: {}", remote_url, err)
        })?;

    Ok(())
}
