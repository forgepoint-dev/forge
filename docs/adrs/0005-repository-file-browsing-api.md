# ADR-0005: Repository File Browsing API

- Status: Accepted
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Context

Forgepoint needs to provide read-only access to repository files and directory structures through the GraphQL API. Users need to:
- List directory contents with file/folder metadata
- Read file contents for preview and display
- Navigate repository trees across different branches
- Support both local and remote (cached) repositories

The implementation must be performant for typical repository sizes while preventing abuse (e.g., large file reads, path traversal attacks).

## Decision

We implemented a file browsing API using the `gix` library for Git operations with the following components:

### 1. GraphQL API Surface

The `Repository` type provides three resolvers for file operations:

- **`entries(path: String, branch: String)`**: Lists files and directories at a given path
  - Returns array of `RepositoryEntryNode` with name, path, kind (File/Directory), and size
  - Directories are sorted before files, then alphabetically within each category
  - Returns empty array for repositories without commits

- **`file(path: String!, branch: String)`**: Reads file contents
  - Returns `RepositoryFilePayload` with path, name, size, binary detection, text, and truncation flag
  - Text preview limited to 128KB to prevent excessive memory usage
  - Binary files return `is_binary: true` with no text content
  - Rejects requests for directories

- **`branches()`**: Lists all branches in the repository
  - Returns array of `RepositoryBranch` with name, reference, target commit, and default flag
  - Includes both local and remote branches
  - Marks the default branch based on HEAD reference

### 2. Path Normalization and Security

Two normalization functions prevent path traversal attacks:

- **`normalize_tree_path()`**: For directory paths
  - Allows empty path (repository root)
  - Splits on `/` and filters empty segments and `.`
  - Rejects `..` segments (upward traversal)
  - Rejects null bytes
  
- **`normalize_file_path()`**: For file paths
  - Requires non-empty path
  - Rejects empty segments (enforces well-formed paths)
  - Rejects `..` segments and null bytes
  - Ensures path references a file, not root

### 3. Repository Storage Abstraction

The `RepositoryStorage` struct handles both local and remote repositories:

- **Local repositories**: Direct filesystem access to working copies under `FORGE_REPOS_PATH`
- **Remote repositories**: Cached clones under a separate cache directory
  - Cache is refreshed on each access using `gix::prepare_clone()`
  - Old cache is removed before refresh to ensure clean state
  - Background task performs blocking Git operations to avoid blocking async runtime

### 4. Branch Resolution

Branch lookup supports multiple reference formats:
- Short names: `main` → `refs/heads/main` or `refs/remotes/main`
- Full references: `refs/heads/feature` (used directly)
- Falls back to HEAD when branch is not specified
- Handles detached HEAD state

### 5. Performance Considerations

- Git operations run in `tokio::task::spawn_blocking()` to prevent blocking the async runtime
- File preview truncated at 128KB to limit memory usage
- Binary detection via UTF-8 validation (efficient for most files)
- Directory entries sorted in-memory (acceptable for typical repository sizes)

## Consequences

### Positive

- **Security**: Path normalization prevents directory traversal attacks
- **Performance**: Blocking task spawning keeps the async runtime responsive
- **Flexibility**: Supports both local and remote repositories transparently
- **User Experience**: Binary detection and truncation provide good defaults for file preview
- **Compatibility**: Works with any Git repository structure

### Negative

- **Cache Overhead**: Remote repositories are re-cloned on every access (no persistence)
  - Future work: Implement smart cache invalidation using fetch + pull
- **Memory Usage**: Large directories load entire entry list into memory
  - Acceptable for most repositories, but may need pagination for massive trees
- **No Streaming**: Large file reads allocate entire blob in memory
  - 128KB limit mitigates this, but full file download not supported
- **No Write Operations**: Browsing is read-only (Git write operations deferred to future work)

### Trade-offs

- **Truncation vs Completeness**: Chose 128KB limit for preview over full file access
  - Rationale: GraphQL API is for UI display, not file transfer
- **Re-clone vs Cache Staleness**: Chose fresh clone over stale cache
  - Rationale: Correctness over performance for remote repositories
- **Blocking Tasks vs Native Async**: Used `gix` in blocking tasks instead of pure async Git library
  - Rationale: `gix` is mature and feature-complete; async overhead not worth custom implementation

## Implementation Details

### File Structure

- `server/src/repository/entries.rs`: Core Git operations and normalization
- `server/src/repository/queries.rs`: GraphQL query implementations
- `server/src/repository/storage.rs`: Repository path resolution and caching
- `server/src/repository/models.rs`: Data structures for entries, files, and branches

### Error Handling

- Path traversal attempts return GraphQL errors with `BAD_USER_INPUT` code
- Missing files/directories return descriptive errors
- Git operation failures bubble up as `INTERNAL_SERVER_ERROR`
- Binary files accessed as text return `is_binary: true` rather than error

## Future Enhancements

1. **Smart Remote Caching**: Replace re-clone with `git fetch` + merge detection
2. **Streaming File Download**: Add separate endpoint for full file retrieval
3. **Directory Pagination**: Add cursor-based pagination for large directories
4. **Blob Links**: Return raw blob URLs for direct file access
5. **Commit History**: Add file history and blame information
6. **Diff Support**: Add file diffs between commits/branches
7. **Search**: Add code search within repositories

## References

- `gix` library documentation: https://docs.rs/gix/
- GraphQL schema: `server/src/repository/queries.rs`
- Path normalization tests: `server/src/repository/entries.rs` (test module)
