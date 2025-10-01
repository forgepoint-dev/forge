# Cloning Repositories in Forgepoint

## Overview

Forgepoint now supports cloning remote repositories via HTTPS using the `cloneRepository` GraphQL mutation. This feature allows you to immediately clone a repository from platforms like GitHub, GitLab, or any Git server accessible via HTTPS.

## Mutations Available

### 1. `linkRemoteRepository` (Lazy Loading)

Creates a database record pointing to a remote repository. The actual cloning happens lazily when you first access the repository (e.g., when browsing files).

```graphql
mutation {
  linkRemoteRepository(url: "https://github.com/user/repo") {
    id
    slug
    remoteUrl
    isRemote
  }
}
```

**Use case**: When you want to register a remote repository but don't need immediate access to its contents.

### 2. `cloneRepository` (Immediate Cloning)

Creates a database record AND immediately clones the repository to local storage. The repository is available for browsing right after the mutation completes.

```graphql
mutation {
  cloneRepository(url: "https://github.com/user/repo") {
    id
    slug
    remoteUrl
    isRemote
  }
}
```

**Use case**: When you need immediate access to the repository contents or want to ensure the repository is available offline.

## Requirements

- **HTTPS URLs only**: SSH URLs (e.g., `git@github.com:user/repo.git`) are not supported
- **No authentication**: Currently, only public repositories can be cloned
- **Valid Git repository**: The URL must point to a valid Git repository

## URL Examples

### Supported URLs

```
https://github.com/octocat/Hello-World
https://github.com/octocat/Hello-World.git
https://gitlab.com/user/project
https://bitbucket.org/user/repository
```

### Unsupported URLs

```
git@github.com:user/repo.git          # SSH not supported
http://example.com/not-a-repo         # Not a Git repository
ssh://git@github.com/user/repo        # SSH protocol not supported
```

## Complete Example Workflow

### 1. Clone a Repository

```graphql
mutation {
  cloneRepository(url: "https://github.com/torvalds/linux") {
    id
    slug
    remoteUrl
    isRemote
  }
}
```

Response:
```json
{
  "data": {
    "cloneRepository": {
      "id": "repo_abc123xyz",
      "slug": "linux",
      "remoteUrl": "https://github.com/torvalds/linux",
      "isRemote": true
    }
  }
}
```

### 2. Browse the Cloned Repository

```graphql
query {
  getRepository(path: "linux") {
    id
    slug
    isRemote
  }
}
```

### 3. List Repository Files

```graphql
query {
  browseRepository(path: "linux", treePath: "") {
    treePath
    entries {
      name
      type
      path
      size
    }
  }
}
```

### 4. Read a File from the Repository

```graphql
query {
  readRepositoryFile(
    path: "linux"
    filePath: "README"
    branch: "refs/heads/master"
  ) {
    path
    name
    size
    text
    isBinary
    truncated
  }
}
```

### 5. List Available Branches

```graphql
query {
  listRepositoryBranches(path: "linux") {
    name
    reference
    target
    isDefault
  }
}
```

## Error Handling

The mutation will fail with appropriate error messages in these cases:

### Invalid URL
```json
{
  "errors": [
    {
      "message": "invalid remote repository URL"
    }
  ]
}
```

### SSH URL (Not Supported)
```json
{
  "errors": [
    {
      "message": "only http(s) remote URLs are supported"
    }
  ]
}
```

### Repository Already Cloned
```json
{
  "errors": [
    {
      "message": "remote repository already linked"
    }
  ]
}
```

### Clone Failed (Network, Invalid Repository, etc.)
```json
{
  "errors": [
    {
      "message": "failed to clone remote repository https://github.com/user/repo: ..."
    }
  ]
}
```

## Performance Considerations

### Cloning Large Repositories

Cloning large repositories (like the Linux kernel) can take several minutes. The mutation will block until the clone is complete. For better user experience:

1. Use `linkRemoteRepository` for large repositories if immediate access isn't needed
2. Consider showing a loading indicator in your UI
3. The clone operation timeout is determined by your GraphQL server settings

### Storage Requirements

Each cloned repository is stored in the `remote_cache_root` directory (configured via `FORGE_REPOS_PATH` environment variable). Ensure you have sufficient disk space:

```bash
# Check available space
df -h /path/to/forge/repos

# Example configuration
export FORGE_REPOS_PATH=/mnt/storage/.forge/repos
```

## Implementation Details

### Slug Generation

The slug (repository identifier in Forgepoint) is automatically generated from the repository URL:

- `https://github.com/user/my-repo` → slug: `my-repo`
- `https://github.com/user/my-repo.git` → slug: `my-repo`

Slugs must be unique at the root level (or within a group if you extend the mutation to support groups).

### Storage Location

Cloned repositories are stored at:
```
{FORGE_REPOS_PATH}/remote_cache/{repository_id}/
```

Each repository gets a unique ID (CUID) to avoid conflicts.

### Difference from `linkRemoteRepository`

| Aspect | `linkRemoteRepository` | `cloneRepository` |
|--------|------------------------|-------------------|
| Database record | ✅ Created immediately | ✅ Created immediately |
| Clone operation | ⏱️ Deferred (on first access) | ✅ Immediate |
| Response time | Fast (~100ms) | Slow (depends on repo size) |
| Use case | Reference, lazy loading | Immediate access required |

## Future Enhancements

### Authentication Support (Planned)

Future versions will support authenticated cloning via ATProto OAuth:

```graphql
mutation {
  cloneRepository(
    url: "https://github.com/user/private-repo"
    auth: {
      type: ATPROTO_OAUTH
      token: "..."
    }
  ) {
    id
    slug
  }
}
```

### Progress Tracking (Planned)

For long-running clone operations:

```graphql
mutation {
  cloneRepository(url: "https://github.com/torvalds/linux") {
    operationId
    status
  }
}

query {
  cloneStatus(operationId: "op_123") {
    progress
    bytesReceived
    bytesTotal
    status
  }
}
```

## Troubleshooting

### "repository directory not found" Error

This typically occurs with `linkRemoteRepository` when trying to access the repository before it has been cloned. Use `cloneRepository` instead to ensure immediate availability.

### Clone Hangs or Times Out

- Check your network connection
- Verify the repository URL is accessible
- For very large repositories, consider increasing the GraphQL timeout
- Try using `linkRemoteRepository` if immediate access isn't required

### "slug already exists" Error

A repository with that slug already exists. You can:
1. Check existing repositories: `query { getAllRepositories { slug remoteUrl } }`
2. Delete the existing repository if it's no longer needed
3. The URL normalization might differ from what you expect

## Testing

To test the cloning functionality, use a small public repository:

```graphql
# Small test repository
mutation {
  cloneRepository(url: "https://github.com/octocat/Hello-World") {
    id
    slug
    remoteUrl
  }
}

# Verify it was cloned
query {
  getRepository(path: "hello-world") {
    id
    isRemote
  }
}

# Browse files
query {
  browseRepository(path: "hello-world", treePath: "") {
    entries {
      name
      type
    }
  }
}
```

## Related Documentation

- [GraphQL API Reference](../CLAUDE.md#graphql-playground)
- [Repository Storage](../CLAUDE.md#file-operations)
- [Architecture Decision Records](../docs/adrs/)
