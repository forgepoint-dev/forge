# RFC-0002: Git Operations Integration

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Integrate core Git operations (clone, push, pull, fetch) into Forgepoint to enable full repository lifecycle management. Transform Forgepoint from a metadata-only forge into a complete Git hosting platform.

## Motivation

Currently, Forgepoint provides:
- Repository metadata management (groups, slugs, paths)
- Read-only file browsing via GraphQL API
- Remote repository caching (clone on access)
- Directory and file listing

However, Forgepoint **cannot**:
- Accept Git pushes from clients
- Create Git repositories with initial commits
- Serve Git data via Git protocols (HTTP, SSH)
- Handle branch/tag creation and deletion
- Manage Git hooks for workflows

To become a viable Gitea alternative, Forgepoint must support the complete Git workflow: `git clone` → edit → `git push` → collaborate.

## Terminology

- **Git Smart HTTP**: Git protocol over HTTP using `git-upload-pack` and `git-receive-pack` services
- **Git SSH**: Git protocol over SSH using `git-upload-pack` and `git-receive-pack` commands
- **Working Copy**: Local repository filesystem with `.git` directory and checked-out files
- **Bare Repository**: Git repository without working directory (contains only `.git` internals)
- **Reference**: Git branch, tag, or symbolic reference (HEAD)

## Proposal

Implement Git operations support in three phases:

### Phase 1: Git HTTP Server (Read-Only)

Enable `git clone` and `git fetch` over HTTP.

**Requirements:**
- HTTP endpoint: `GET /{owner}/{repo}/info/refs?service=git-upload-pack`
- HTTP endpoint: `POST /{owner}/{repo}/git-upload-pack`
- Serve Git pack protocol responses
- Support both local and remote repositories
- Authentication (future): Read access control

**Implementation:**
- Use `gix` library for pack generation
- Axum routes for Git endpoints
- Convert repository paths to filesystem paths
- Stream pack data efficiently

**Limitations:**
- Read-only (no push support)
- No SSH support
- No Git hooks
- No protocol v2 (start with v1)

### Phase 2: Git HTTP Server (Write Support)

Enable `git push` over HTTP.

**Requirements:**
- HTTP endpoint: `POST /{owner}/{repo}/git-receive-pack`
- Accept incoming pack data
- Update references (branches, tags)
- Validate push permissions
- Reject force pushes (configurable)
- Support atomic transactions

**Implementation:**
- Parse Git pack protocol for receive-pack
- Use `gix` for pack parsing and object insertion
- Implement reference updates with locking
- Add pre-receive and post-receive hook support
- Transaction rollback on errors

**Challenges:**
- Concurrent push handling (locking strategy)
- Large pack performance
- Hook execution model (WASM? External?)
- Quota enforcement

### Phase 3: Git SSH Server

Add SSH protocol support for power users.

**Requirements:**
- SSH server on port 2222 (configurable)
- Public key authentication
- Execute `git-upload-pack` and `git-receive-pack` commands
- Chroot to repository directories
- User-specific SSH keys

**Implementation:**
- Use `russh` or `thrussh` for SSH server
- Parse SSH commands (`git-upload-pack`, `git-receive-pack`)
- Delegate to same Git backend as HTTP
- Store SSH public keys in database
- Map SSH user to Forge user (future: multi-user support)

**Challenges:**
- SSH key management
- User isolation (single-user vs multi-user)
- Port conflicts (2222 vs standard 22)
- Docker/container compatibility

## Design Decisions

### 1. Repository Storage Model

**Decision:** Convert to **bare repositories** for Git hosting, keep working copies separate.

**Rationale:**
- Bare repos are the Git hosting standard
- No working directory confusion
- Cleaner reference management
- Matches GitHub/GitLab/Gitea architecture

**Impact:**
- Migration: Convert existing working copies to bare repos
- Two directories: `FORGE_REPOS_PATH` (bare) + `FORGE_WORKING_COPIES_PATH` (optional checkouts)
- File browsing reads from bare repo (current behavior preserved)

### 2. Git Library Choice

**Decision:** Use **`gix`** (gitoxide) instead of `git2` (libgit2).

**Rationale:**
- Pure Rust (no C dependencies)
- Better async support
- Modern API design
- Active development
- Already used for file browsing

**Alternatives Considered:**
- `git2-rs`: Mature but based on libgit2 (C dependency, less async-friendly)
- Shell out to `git`: Simple but slower, harder to control, platform-dependent

### 3. Protocol Support Order

**Decision:** HTTP first, SSH later (Phase 3).

**Rationale:**
- HTTP is easier to implement (Axum integration)
- HTTP works in more network environments (firewalls)
- HTTPS provides encryption without SSH complexity
- SSH can be added later for power users

### 4. Authentication Integration

**Decision:** Defer authentication to Phase 2 (authentication PRD).

**Rationale:**
- Git operations require stable auth system
- Single-user mode can skip auth initially
- HTTP Basic Auth or Bearer tokens when ready
- SSH keys tied to user accounts

### 5. Git Hooks

**Decision:** Support hooks via **WASM extensions**.

**Rationale:**
- Consistent with Forgepoint extension model
- Sandboxed execution (security)
- Cross-platform (no shell scripts)
- Can access Forge database and API

**Challenges:**
- WASM can't block Git operations (async model)
- Need hook API design (separate RFC)

## API Design

### GraphQL Schema Changes

**New Mutation:**
```graphql
"""
Initialize a repository with an initial commit (README, .gitignore)
"""
mutation {
  initializeRepository(
    input: {
      repositoryId: ID!
      defaultBranch: String = "main"
      createReadme: Boolean = true
      gitignoreTemplate: String
    }
  ): Repository!
}
```

**New Query:**
```graphql
"""
Get Git clone URLs for a repository
"""
query {
  getRepository(path: String!) {
    cloneUrls {
      http: String!
      ssh: String
    }
  }
}
```

### HTTP Endpoints

```
GET  /{owner}/{repo}/info/refs?service=git-upload-pack
POST /{owner}/{repo}/git-upload-pack
POST /{owner}/{repo}/git-receive-pack
```

**Authentication:**
- HTTP Basic Auth: `Authorization: Basic base64(username:token)`
- Bearer Token: `Authorization: Bearer <token>`

**Response Format:**
- Git pack protocol (binary)
- Content-Type: `application/x-git-upload-pack-result` or `application/x-git-receive-pack-result`

## Implementation Plan

### Phase 1: Git HTTP Server (Read-Only)

**Deliverables:**
1. Axum routes for `info/refs` and `git-upload-pack`
2. `gix` integration for pack generation
3. Repository path resolution (path → filesystem)
4. Integration tests: `git clone`, `git fetch`, `git pull`
5. Documentation: How to clone from Forgepoint

**Estimated Effort:** 2-3 weeks

### Phase 2: Git HTTP Server (Write Support)

**Deliverables:**
1. Axum route for `git-receive-pack`
2. `gix` integration for pack parsing and object insertion
3. Reference updates with locking
4. Pre-receive and post-receive hook framework
5. Integration tests: `git push`, force push rejection
6. Documentation: How to push to Forgepoint

**Estimated Effort:** 3-4 weeks

### Phase 3: Git SSH Server

**Deliverables:**
1. SSH server implementation (`russh`)
2. Command parsing (`git-upload-pack`, `git-receive-pack`)
3. SSH key management (database schema, API)
4. User authentication via SSH keys
5. Integration tests: SSH clone, push
6. Documentation: SSH key setup

**Estimated Effort:** 2-3 weeks

## Security Considerations

1. **Path Traversal Prevention**
   - Validate repository paths (no `..`, `/etc`, etc.)
   - Sandboxed filesystem access
   - Already implemented in current file browsing

2. **Denial of Service**
   - Rate limiting on Git endpoints
   - Maximum pack size enforcement
   - Timeout on long-running operations

3. **Repository Access Control**
   - Public/private repository flags (future)
   - Per-repository read/write permissions
   - Integration with auth system (Phase 2)

4. **Git Hook Security**
   - WASM sandboxing (memory limits, fuel limits)
   - No filesystem access outside repository
   - No network access (except Forge API)

5. **SSH Key Security**
   - Hash SSH keys before storage
   - Key fingerprint validation
   - Revocation mechanism

## Performance Considerations

1. **Pack Generation**
   - Stream packs instead of buffering
   - Use `gix` incremental pack generation
   - Cache pack files for popular commits

2. **Concurrent Access**
   - Read locks for fetch operations
   - Write locks for push operations
   - Per-repository locking (not global)

3. **Large Repositories**
   - Shallow clone support
   - Partial clone support (Git LFS future)
   - Pack file reuse across fetches

## Compatibility

- **Git Client Versions:** Support Git 2.30+ (protocol v1)
- **Transport:** HTTP/1.1 and HTTP/2
- **Encoding:** UTF-8 for references, binary for pack data
- **Large Files:** No LFS support initially (future enhancement)

## Testing Strategy

1. **Unit Tests**
   - Path resolution
   - Pack generation
   - Reference updates

2. **Integration Tests**
   - Real Git client operations (`git clone`, `git push`)
   - Multi-client scenarios (concurrent clone)
   - Error cases (invalid refs, bad packs)

3. **Performance Tests**
   - Large repository clones
   - Concurrent push/fetch
   - Memory usage profiling

## Open Questions

1. **How should we handle Git LFS?**
   - Defer to future RFC?
   - Third-party LFS server?
   - Built-in support?

2. **What's the migration path for existing repositories?**
   - Auto-convert working copies to bare repos?
   - Manual migration step?
   - Keep both formats?

3. **How do hooks integrate with extensions?**
   - New extension API for hooks?
   - Separate hook system?
   - Part of existing extension context?

4. **Should we support Git protocol (git://)?**
   - HTTP + SSH should be sufficient
   - Git protocol is legacy (no encryption)

5. **How to handle repository forks?**
   - Hard links for object sharing?
   - Separate full clones?
   - Fork metadata tracking?

## Success Criteria

- Users can `git clone` from Forgepoint over HTTP
- Users can `git push` to Forgepoint over HTTP
- Users can `git clone` from Forgepoint over SSH (Phase 3)
- Performance comparable to Gitea (clones < 1s for small repos)
- No data corruption under concurrent access
- Comprehensive test coverage (>80%)

## References

- Git Pack Protocol: https://git-scm.com/docs/pack-protocol
- Git HTTP Transport: https://git-scm.com/docs/http-protocol
- `gix` library: https://docs.rs/gix/
- Gitea implementation: https://github.com/go-gitea/gitea
- Current file browsing: `server/src/repository/entries.rs`

## Future Enhancements

- Git protocol v2 support (improved fetch performance)
- Git LFS support (large file storage)
- Git submodules optimization
- Garbage collection automation
- Repository mirroring (sync with external repos)
- Signed commits verification (GPG)
