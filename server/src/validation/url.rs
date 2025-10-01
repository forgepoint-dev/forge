use url::Url;

pub fn normalize_remote_repository(raw_url: &str) -> anyhow::Result<(String, String)> {
    let mut url =
        Url::parse(raw_url).map_err(|_| anyhow::anyhow!("invalid remote repository URL"))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(anyhow::anyhow!("only http(s) remote URLs are supported")),
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

fn slug_from_remote_url(url: &Url) -> anyhow::Result<String> {
    let segments: Vec<_> = url
        .path_segments()
        .ok_or_else(|| anyhow::anyhow!("remote URL is missing path segments"))?
        .filter(|segment| !segment.is_empty())
        .collect();

    let Some(last_segment) = segments.last() else {
        return Err(anyhow::anyhow!("remote URL must include a repository name"));
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
        return Err(anyhow::anyhow!(
            "repository name in URL cannot be converted to a valid slug",
        ));
    }

    Ok(slug)
}
