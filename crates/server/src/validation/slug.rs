pub fn validate_slug(slug: &str) -> anyhow::Result<()> {
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
        Err(anyhow::anyhow!("slug must be lowercase kebab-case"))
    }
}
