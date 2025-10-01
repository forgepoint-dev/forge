# Repository Cloning: Feature Comparison

## Visual Flow Comparison

### `linkRemoteRepository` (Existing - Lazy Loading)

```
User Request (Mutation)
    ↓
Validate URL ✓
    ↓
Create DB Record ✓
    ↓
Return Response (Fast ~100ms)
    ↓
[Later: User browses repository]
    ↓
Clone on First Access (Slow)
    ↓
Cache for Future Use
```

**Pros:**
- Fast response time
- User doesn't wait for cloning
- Good for "bookmark" use cases

**Cons:**
- First access is slow
- Repository might fail to clone when user tries to access it
- Not available offline immediately


### `cloneRepository` (NEW - Immediate Cloning)

```
User Request (Mutation)
    ↓
Validate URL ✓
    ↓
Create DB Record ✓
    ↓
Clone Repository NOW (Slow - minutes for large repos)
    ↓
Return Response with Complete Data
    ↓
[User browses repository]
    ↓
Access Cached Clone (Fast)
```

**Pros:**
- Repository ready immediately after mutation
- Errors detected during mutation (better UX)
- Available offline after cloning
- Fast subsequent access

**Cons:**
- Slow mutation response (user must wait)
- Blocks mutation until complete


## Use Case Examples

### Scenario 1: Reference Library
**Best Choice:** `linkRemoteRepository`

```graphql
# Add 100 repos to your library quickly
mutation {
  linkRemoteRepository(url: "https://github.com/user/repo1") { id }
  linkRemoteRepository(url: "https://github.com/user/repo2") { id }
  # ... etc
}
```

You may never look at repo50, so why clone it?


### Scenario 2: Active Development
**Best Choice:** `cloneRepository`

```graphql
# Clone a dependency you're actively working with
mutation {
  cloneRepository(url: "https://github.com/expressjs/express") {
    id
    slug
  }
}

# Immediately browse it
query {
  browseRepository(path: "express", treePath: "lib") {
    entries { name type }
  }
}
```

The repository is ready for immediate exploration.


### Scenario 3: Offline Work
**Best Choice:** `cloneRepository`

Clone repositories before going offline:

```graphql
mutation {
  cloneRepository(url: "https://github.com/rust-lang/rust") { id }
  cloneRepository(url: "https://github.com/golang/go") { id }
}
```

Now you can browse them without internet.


## Timeline Visualization

### Timeline: linkRemoteRepository

```
t=0s    User submits mutation
t=0.1s  ✅ Mutation completes (DB record created)
        User browses UI
t=10s   User clicks "Browse Files"
t=10s   ⏳ Cloning starts in background
t=65s   ✅ Repository available
```

Total time to access: **65 seconds**
Perceived wait: **55 seconds** (during file browse)


### Timeline: cloneRepository

```
t=0s    User submits mutation
t=0s    ⏳ Cloning starts immediately
t=55s   ✅ Mutation completes (repository cloned)
        User browses UI
t=60s   User clicks "Browse Files"
t=60s   ✅ Repository available (instant - uses cache)
```

Total time to access: **60 seconds**
Perceived wait: **55 seconds** (during mutation)


## Technical Implementation Comparison

### linkRemoteRepository

```rust
pub async fn link_remote_repository_raw(
    pool: &SqlitePool,
    _storage: &RepositoryStorage,  // Not used!
    url: String,
) -> anyhow::Result<RepositoryRecord> {
    // 1. Validate URL
    let (normalized_url, slug) = normalize_remote_repository(&url)?;
    
    // 2. Check duplicates
    if remote_url_exists(pool, &normalized_url).await? {
        return Err(anyhow::anyhow!("remote repository already linked"));
    }
    
    // 3. Create DB record
    let id = cuid2::create_id();
    sqlx::query("INSERT INTO repositories ...")
        .bind(&id)
        .bind(&slug)
        .bind(&normalized_url)
        .execute(pool)
        .await?;
    
    // 4. Return (NO CLONING YET!)
    Ok(RepositoryRecord { ... })
}
```

### cloneRepository

```rust
pub async fn clone_repository_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,  // Used for cloning!
    url: String,
) -> anyhow::Result<RepositoryRecord> {
    // 1-3. Same as linkRemoteRepository
    // (validate, check duplicates, create DB record)
    
    let record = RepositoryRecord { ... };
    
    // 4. IMMEDIATELY CLONE! (This is the key difference)
    storage.ensure_remote_repository(&record).await?;
    
    // 5. Return with repository already cloned
    Ok(record)
}
```

**Key Difference:** Line 4 - the `storage.ensure_remote_repository()` call


## API Design Decision

### Why Two Separate Mutations?

**Alternative 1 (Rejected):** Single mutation with flag
```graphql
mutation {
  linkRemoteRepository(url: "...", immediate: true)
}
```

❌ Problems:
- Unclear semantics
- Optional parameter can be forgotten
- Mutation behavior changes dramatically based on a boolean

**Alternative 2 (Chosen):** Separate mutations
```graphql
mutation {
  linkRemoteRepository(url: "...")  # Lazy
  cloneRepository(url: "...")       # Immediate
}
```

✅ Benefits:
- Clear intent from mutation name
- Different timeouts can be set per mutation
- Better GraphQL schema documentation
- Follows principle: "Make the right thing easy"


## Error Handling Comparison

### linkRemoteRepository
```
Mutation → Success → User browses → Clone fails → Error shown to user
```
Error happens later, potentially confusing the user


### cloneRepository
```
Mutation → Clone fails → Error in mutation response
```
Error happens immediately, clear cause and effect


## Recommendations

| Your Situation | Use This |
|----------------|----------|
| Building a repository catalog/library | `linkRemoteRepository` |
| Need to browse files immediately | `cloneRepository` |
| Uncertain if you'll use the repo | `linkRemoteRepository` |
| Working offline later | `cloneRepository` |
| Adding many repos at once | `linkRemoteRepository` |
| Repository is critical for next step | `cloneRepository` |
| Large repository (>1GB) | `linkRemoteRepository` |
| Small repository (<10MB) | `cloneRepository` |


## Performance Characteristics

| Aspect | linkRemoteRepository | cloneRepository |
|--------|---------------------|-----------------|
| Mutation response time | ~100ms | Depends on repo size (seconds to minutes) |
| First file access | Slow (must clone) | Fast (already cloned) |
| Database size | Same | Same |
| Disk usage | Less (if never accessed) | More (always clones) |
| Network bandwidth | Same eventual usage | Same eventual usage |
| Error visibility | Delayed | Immediate |


## Migration Path

If you have repositories linked with `linkRemoteRepository` and want them cloned:

```graphql
# Query existing remote repositories
query {
  getAllRepositories {
    id
    slug
    remoteUrl
    isRemote
  }
}

# For each one, the next time it's accessed, it will be cloned automatically
# Or manually trigger browsing to force clone:
query {
  browseRepository(path: "repo-slug", treePath: "") {
    entries { name }
  }
}
```

There's no migration needed - both approaches work together seamlessly!
