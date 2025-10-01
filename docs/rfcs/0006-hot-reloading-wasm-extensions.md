# RFC-0006: Hot-Reloading WASM Extensions

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Enable hot-reloading of WASM extensions during development to improve developer experience. Extensions can be reloaded without restarting the server, preserving state where possible.

## Motivation

Currently, modifying a WASM extension requires:
1. Rebuild the WASM module
2. Stop the Forgepoint server
3. Restart the server
4. Wait for initialization
5. Re-run GraphQL queries

This workflow is slow and disrupts development flow. For a tight iteration cycle, developers need:
- **Fast Feedback**: See changes in <5 seconds
- **Preserved State**: Don't lose database state on reload
- **Incremental Updates**: Reload only changed extensions
- **Error Recovery**: Handle reload failures gracefully

Hot-reloading is essential for productive extension development.

## Terminology

- **Hot Reload**: Replace running extension with new version without server restart
- **Watch Mode**: Monitor filesystem for changes, trigger reload automatically
- **Extension State**: SQLite database and in-memory cache
- **Schema Update**: GraphQL schema regeneration after reload
- **Graceful Failure**: Reload fails but server continues running

## Proposal

Implement hot-reloading in three phases:

### Phase 1: Manual Reload

Add API endpoint to reload specific extension.

**Requirements:**
- GraphQL mutation: `reloadExtension(name: String!): Extension!`
- REST endpoint: `POST /_forge/extensions/{name}/reload`
- Reload WASM module from filesystem
- Re-initialize extension with same database
- Update GraphQL schema
- Validate new schema is compatible

**Workflow:**
1. Developer modifies extension code
2. Developer runs `just build` to compile WASM
3. Developer calls `reloadExtension(name: "issues")`
4. Server reloads extension and responds with status
5. Developer tests changes in GraphQL Playground

### Phase 2: File Watching

Automatically reload when WASM file changes.

**Requirements:**
- Watch extension directories for `.wasm` file changes
- Debounce rapid changes (compile takes time)
- Reload automatically after file stabilizes
- Log reload events
- Handle compile errors gracefully

**Workflow:**
1. Developer runs server with `--watch` flag
2. Developer modifies extension code
3. Developer runs `just build` to compile WASM
4. Server detects `.wasm` file change
5. Server automatically reloads extension
6. Developer sees reload log in terminal
7. Developer refreshes GraphQL Playground

### Phase 3: Live Reload Client

Notify frontend when extensions reload.

**Requirements:**
- WebSocket endpoint: `ws://localhost:8000/_forge/live-reload`
- Send reload event to connected clients
- Frontend reloads GraphQL schema
- Frontend re-fetches data if needed
- Developer doesn't need to refresh browser

**Workflow:**
1. Developer runs server with `--watch` flag
2. Frontend connects to WebSocket
3. Developer modifies extension code
4. Server reloads extension
5. Server sends WebSocket event: `{type: "extension-reload", name: "issues"}`
6. Frontend receives event, refetches GraphQL schema
7. Frontend re-runs active queries
8. Developer sees updated data instantly

## Implementation Details

### Extension Reload Process

```rust
async fn reload_extension(name: &str) -> anyhow::Result<()> {
    // 1. Find extension by name
    let extension = find_extension(name)?;
    
    // 2. Load new WASM module
    let new_module = load_wasm_module(&extension.path).await?;
    
    // 3. Validate schema compatibility
    let new_schema = extract_schema(&new_module)?;
    validate_schema_compatible(&extension.schema, &new_schema)?;
    
    // 4. Gracefully shutdown old instance
    shutdown_extension(&extension).await?;
    
    // 5. Initialize new instance with existing database
    let new_instance = initialize_extension(
        new_module,
        &extension.database_path
    ).await?;
    
    // 6. Update GraphQL schema
    update_graphql_schema(name, &new_schema).await?;
    
    // 7. Replace extension in registry
    replace_extension(name, new_instance).await?;
    
    Ok(())
}
```

### File Watching

Use `notify` crate for filesystem watching:

```rust
use notify::{Watcher, RecursiveMode, Event};

async fn watch_extensions(extensions_dir: PathBuf) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            let _ = tx.blocking_send(event);
        }
    })?;
    
    watcher.watch(&extensions_dir, RecursiveMode::Recursive)?;
    
    while let Some(event) = rx.recv().await {
        if event.kind.is_modify() {
            for path in event.paths {
                if path.extension() == Some("wasm") {
                    if let Some(name) = extract_extension_name(&path) {
                        // Debounce: Wait for file to stabilize
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        
                        match reload_extension(&name).await {
                            Ok(_) => info!("Reloaded extension: {}", name),
                            Err(e) => error!("Failed to reload {}: {}", name, e),
                        }
                    }
                }
            }
        }
    }
}
```

### Schema Compatibility Check

Ensure new schema doesn't break existing queries:

```rust
fn validate_schema_compatible(old: &Schema, new: &Schema) -> anyhow::Result<()> {
    // Check for removed types
    for old_type in &old.types {
        if !new.types.contains(old_type) {
            return Err(anyhow!("Type {} was removed", old_type.name));
        }
    }
    
    // Check for removed fields
    for old_type in &old.types {
        if let Some(new_type) = new.types.find(&old_type.name) {
            for old_field in &old_type.fields {
                if !new_type.fields.contains(old_field) {
                    return Err(anyhow!(
                        "Field {}.{} was removed",
                        old_type.name,
                        old_field.name
                    ));
                }
            }
        }
    }
    
    // Allow:
    // - Adding new types
    // - Adding new fields
    // - Changing field descriptions
    // - Adding arguments (with defaults)
    
    Ok(())
}
```

### WebSocket Live Reload

```rust
use axum::extract::ws::{WebSocket, Message};

async fn live_reload_handler(ws: WebSocket) {
    let (mut sender, _) = ws.split();
    
    let mut rx = subscribe_to_reload_events();
    
    while let Some(event) = rx.recv().await {
        let message = serde_json::to_string(&event).unwrap();
        if sender.send(Message::Text(message)).await.is_err() {
            break;
        }
    }
}
```

**Frontend:**
```typescript
const ws = new WebSocket('ws://localhost:8000/_forge/live-reload');

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  
  if (data.type === 'extension-reload') {
    console.log(`Extension ${data.name} reloaded`);
    
    // Refetch GraphQL schema
    await refetchSchema();
    
    // Re-run active queries
    await refetchQueries();
  }
};
```

## Design Decisions

### 1. Reload vs Restart

**Decision:** Implement **hot reload** instead of requiring server restart.

**Rationale:**
- Faster iteration (seconds vs minutes)
- Preserves server state (other extensions, cache)
- Preserves database state (no migrations needed)
- Better developer experience

**Trade-offs:**
- More complex implementation
- Risk of stale state
- Need schema compatibility checks

### 2. Manual vs Automatic

**Decision:** Support **both manual and automatic** reload.

**Rationale:**
- Manual: Explicit control, useful for debugging
- Automatic: Faster workflow, hands-free development
- Different use cases prefer different modes

### 3. Schema Compatibility

**Decision:** **Validate schema compatibility**, fail if breaking changes.

**Rationale:**
- Prevent breaking active queries
- Ensure type safety
- Clear error messages

**Allowed Changes:**
- Add new types
- Add new fields
- Add optional arguments
- Change descriptions

**Disallowed Changes:**
- Remove types
- Remove fields
- Remove arguments
- Change field types (unless compatible)

### 4. State Preservation

**Decision:** Preserve **database state**, reset in-memory state.

**Rationale:**
- Database is persistent (survives restart anyway)
- In-memory state is ephemeral (can be reconstructed)
- Simplifies implementation

**Implementation:**
- Reuse same SQLite database
- Clear in-memory caches
- Re-initialize extension context

### 5. Error Handling

**Decision:** **Gracefully degrade** on reload failure.

**Rationale:**
- Server should not crash
- Keep old extension running if reload fails
- Show clear error messages

**Behavior:**
- Log error details
- Keep old extension active
- Return error response
- Allow retry

## GraphQL API

### Queries

```graphql
"""
List all extensions
"""
query listExtensions: [Extension!]!

type Extension {
  name: String!
  version: String!
  path: String!
  loaded: Boolean!
  error: String
  schema: String!
}
```

### Mutations

```graphql
"""
Reload a specific extension
"""
mutation reloadExtension(name: String!): Extension!

"""
Reload all extensions
"""
mutation reloadAllExtensions: [Extension!]!

"""
Enable/disable watch mode
"""
mutation setWatchMode(enabled: Boolean!): Boolean!
```

## Configuration

Add watch mode configuration:

```ron
// forge.ron
Config(
    extensions: Extensions(
        // ...
    ),
    development: Development(
        watch: true,            // Enable file watching
        liveReload: true,       // Enable WebSocket live reload
        watchDebounce: 500,     // Milliseconds to wait after file change
    ),
)
```

## CLI Flags

```bash
# Enable watch mode
forgepoint --watch

# Enable live reload (implies watch)
forgepoint --live-reload

# Manual reload via CLI
forgepoint-cli reload-extension issues

# Manual reload via API
curl -X POST http://localhost:8000/_forge/extensions/issues/reload
```

## Testing Strategy

1. **Unit Tests**
   - Schema compatibility validation
   - Extension registry updates
   - Graceful failure handling

2. **Integration Tests**
   - Manual reload workflow
   - Automatic reload on file change
   - WebSocket message delivery
   - Error recovery

3. **E2E Tests**
   - Modify extension source
   - Rebuild WASM
   - Verify reload
   - Test queries still work

## Performance Considerations

- **Reload Time**: Target <2 seconds for small extensions
- **Memory Usage**: Ensure old WASM instances are properly freed
- **File Watching**: Use efficient OS-level APIs (inotify, FSEvents)
- **Debouncing**: Prevent excessive reloads during rapid edits

## Security Considerations

1. **Production Deployment**
   - Disable watch mode in production
   - Disable reload API in production
   - Require authentication for reload endpoint

2. **File Permissions**
   - Validate extension paths (no directory traversal)
   - Only watch configured extension directories

3. **Resource Limits**
   - Rate limit reload requests
   - Maximum reload frequency

## Open Questions

1. **What if schema changes are breaking?**
   - Reject reload?
   - Show warning and allow?
   - Version GraphQL schema?

2. **How to handle extension dependencies?**
   - If extension A depends on B, reload both?
   - Dependency graph tracking?

3. **Should we support rollback?**
   - Keep previous WASM module for rollback?
   - Manual rollback command?

4. **What about OCI-fetched extensions?**
   - Can't watch remote changes
   - Support local override?

5. **How to test hot reload itself?**
   - Test harness for reload scenarios?
   - Mock filesystem changes?

## Success Criteria

- Extension reload completes in <2 seconds
- Database state is preserved across reloads
- Schema compatibility is validated
- File watching triggers reload automatically
- WebSocket notifies frontend of reloads
- No memory leaks from old WASM instances
- Clear error messages on reload failure

## References

- `notify` crate: https://docs.rs/notify/
- Wasmtime module lifecycle: https://docs.wasmtime.dev/
- Hot reloading patterns: https://fasterthanli.me/articles/so-you-want-to-live-reload-rust
- GraphQL schema evolution: https://graphql.org/learn/best-practices/#versioning

## Future Enhancements

- **Incremental compilation**: Faster WASM builds
- **Extension versioning**: Track schema versions
- **Rollback**: Revert to previous extension version
- **Extension marketplace**: Hot-install extensions from registry
- **Multi-instance**: Reload in background, switch atomically
- **Extension profiles**: Different extensions for dev/prod
