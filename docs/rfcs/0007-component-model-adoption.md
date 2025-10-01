# RFC-0007: Component Model Adoption (WASI Preview 2)

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Migrate Forgepoint's WASM extension system from WASI Preview 1 to the WebAssembly Component Model (WASI Preview 2). This enables better language interoperability, improved type safety, and access to modern WASI features.

## Motivation

Forgepoint currently uses WASI Preview 1 for extensions:
- Limited to simple function calls
- No rich type system (only scalars and buffers)
- No first-class async support
- No cross-language component composition
- WASI Preview 1 is being phased out

The Component Model provides:
- **Rich Types**: Records, variants, options, results natively
- **Language Interop**: Compose extensions written in different languages
- **Async Support**: Native async/await in WASM
- **Better Tooling**: `wasm-tools`, `wit-bindgen`, standardized
- **Future-Proof**: Industry standard (W3C, Bytecode Alliance)

Migrating now positions Forgepoint for long-term success.

## Terminology

- **Component Model**: WebAssembly standard for composable, portable components
- **WIT (WASM Interface Type)**: IDL for defining component interfaces
- **WASI Preview 2**: Next generation WASI with Component Model
- **World**: Named collection of imports and exports in WIT
- **Resource**: Opaque handle managed by component
- **wit-bindgen**: Tool to generate language bindings from WIT

## Proposal

Migrate to Component Model in two phases:

### Phase 1: Forge Core Migration

Update Forgepoint server to use Component Model.

**Requirements:**
- Update `wasmtime` to version with Component Model support
- Convert existing WIT definitions to Component Model format
- Update host imports (logging, database) to use Component Model
- Update extension loading to instantiate components
- Maintain backward compatibility with Preview 1 (temporary)

**Changes:**
- `wasmtime` → `wasmtime` with `component-model` feature
- `wasmtime-wasi` → `wasmtime-wasi` with WASI Preview 2
- Custom host bindings → Component Model host implementations

### Phase 2: Extension Migration

Update extensions to use Component Model.

**Requirements:**
- Rewrite extensions using Component Model bindings
- Update build process to use `wasm-tools component new`
- Test all extensions work with new runtime
- Update extension development guide
- Remove Preview 1 compatibility shim

## Component Model Benefits

### Before (WASI Preview 1)

```wit
// preview1/extension.wit
default world extension {
  import log: func(level: u8, message: string)
  import db-query: func(sql: string) -> list<u8>
  
  export init: func(config: string) -> u32
  export schema: func() -> string
  export resolve: func(query: string) -> string
}
```

**Limitations:**
- No structured error handling (error codes)
- JSON serialization for complex types (slow, unsafe)
- No async (blocking calls only)
- No resource management (manual cleanup)

### After (Component Model)

```wit
// component-model/forge-extension.wit
package forge:extension@0.1.0;

interface types {
  record config {
    name: string,
    database-path: string,
  }
  
  variant log-level {
    debug,
    info,
    warn,
    error,
  }
  
  resource database {
    query: func(sql: string) -> result<list<row>, database-error>;
    execute: func(sql: string) -> result<u64, database-error>;
  }
  
  record row {
    columns: list<column-value>,
  }
  
  variant column-value {
    null,
    integer(s64),
    real(float64),
    text(string),
    blob(list<u8>),
  }
  
  record database-error {
    message: string,
    code: u32,
  }
}

interface host {
  use types.{log-level, database};
  
  log: func(level: log-level, message: string);
  database: func() -> database;
}

interface extension {
  use types.{config};
  
  init: func(config: config) -> result<_, string>;
  schema: func() -> string;
  resolve: func(query: string) -> result<string, string>;
}

world forge-extension {
  import host;
  export extension;
}
```

**Benefits:**
- Native error handling with `result<T, E>`
- Type-safe database rows (no JSON parsing)
- Resource management (database handle)
- Structured types (records, variants, options)

## Implementation Details

### Phase 1: Core Migration (4-5 weeks)

**Week 1-2: Wasmtime Upgrade**
- Update `Cargo.toml` dependencies
- Enable Component Model feature
- Update WASI imports to Preview 2
- Test existing extensions (compatibility mode)

**Week 3: WIT Definitions**
- Convert existing WIT to Component Model format
- Define host interfaces (logging, database)
- Define extension interface
- Generate Rust bindings with `wit-bindgen`

**Week 4: Host Implementation**
- Implement host interfaces in Rust
- Wrap SQLite database as resource
- Update extension loader
- Test with new component format

**Week 5: Testing & Documentation**
- Integration tests
- Performance benchmarks
- Update extension development guide
- Migration guide for existing extensions

### Phase 2: Extension Migration (2-3 weeks per extension)

**For each extension:**
1. Update WIT definitions to Component Model
2. Regenerate language bindings
3. Update extension code to use new APIs
4. Build as component: `wasm-tools component new`
5. Test with Forgepoint server
6. Update extension documentation

**Timeline:**
- Issues extension: 2 weeks
- Future extensions: 2-3 weeks each

## New WIT Interface

**Complete WIT Definition:**

```wit
// wit/forge-extension.wit
package forge:extension@0.1.0;

interface types {
  // Configuration
  record extension-config {
    name: string,
    version: string,
    database-path: string,
    settings: option<string>,  // JSON blob
  }
  
  // Logging
  variant log-level {
    debug,
    info,
    warn,
    error,
  }
  
  // Database
  resource database {
    query: func(sql: string) -> result<list<row>, database-error>;
    execute: func(sql: string) -> result<u64, database-error>;
    transaction: func() -> result<transaction, database-error>;
  }
  
  resource transaction {
    query: func(sql: string) -> result<list<row>, database-error>;
    execute: func(sql: string) -> result<u64, database-error>;
    commit: func() -> result<_, database-error>;
    rollback: func() -> result<_, database-error>;
  }
  
  record row {
    columns: list<column-value>,
  }
  
  variant column-value {
    null,
    integer(s64),
    real(float64),
    text(string),
    blob(list<u8>),
  }
  
  record database-error {
    message: string,
    code: option<u32>,
  }
  
  // GraphQL
  record field-context {
    field-name: string,
    parent-type: string,
    arguments: string,  // JSON
  }
}

interface host {
  use types.{log-level, database};
  
  log: func(level: log-level, message: string);
  
  database: func() -> database;
  
  // Future: HTTP client, cron, etc.
}

interface extension {
  use types.{extension-config, field-context};
  
  init: func(config: extension-config) -> result<_, string>;
  
  schema: func() -> result<string, string>;
  
  resolve-field: func(
    context: field-context
  ) -> result<string, string>;
  
  // Lifecycle
  shutdown: func();
}

world forge-extension {
  import host;
  export extension;
}
```

## Migration Path

### Backward Compatibility

During Phase 1, support both Preview 1 and Component Model:

```rust
enum ExtensionKind {
    Preview1(Preview1Extension),
    Component(ComponentExtension),
}

impl Extension {
    fn load(path: &Path) -> Result<ExtensionKind> {
        let bytes = fs::read(path)?;
        
        if is_component(&bytes) {
            Ok(ExtensionKind::Component(load_component(&bytes)?))
        } else {
            Ok(ExtensionKind::Preview1(load_preview1(&bytes)?))
        }
    }
}
```

### Extension Build Process

**Before:**
```bash
cargo build --target wasm32-wasi --release
```

**After:**
```bash
# Build as module
cargo build --target wasm32-wasip2 --release

# Convert to component
wasm-tools component new \
  target/wasm32-wasip2/release/extension.wasm \
  -o extension.component.wasm
```

**Or with `cargo-component`:**
```bash
cargo component build --release
```

## Design Decisions

### 1. Component Model vs Preview 1

**Decision:** Adopt **Component Model** (WASI Preview 2).

**Rationale:**
- Future-proof (industry standard)
- Better type safety
- Better language interoperability
- Async support (future)
- Preview 1 is deprecated

**Trade-offs:**
- Migration effort (rewrite extensions)
- Tooling less mature (improving rapidly)
- Larger WASM binaries (initially)

### 2. Resource-Based Database API

**Decision:** Use **Component Model resources** for database handles.

**Rationale:**
- Type-safe handles (no raw pointers)
- Automatic cleanup
- Transaction support
- Natural async API (future)

**Before (Preview 1):**
```rust
// Manual handle management
let handle = host_db_open();
let result = host_db_query(handle, sql);
host_db_close(handle);  // Easy to forget!
```

**After (Component Model):**
```rust
// Automatic resource management
let db = host::database();
let result = db.query(sql)?;
// db automatically cleaned up
```

### 3. Phased Migration

**Decision:** Migrate core first, then extensions.

**Rationale:**
- Core changes affect all extensions
- Test infrastructure before migrating extensions
- Learn from core migration
- Backward compatibility during transition

### 4. wit-bindgen vs Manual Bindings

**Decision:** Use **wit-bindgen** for code generation.

**Rationale:**
- Type-safe bindings automatically generated
- Consistent across languages
- Maintained by Bytecode Alliance
- Reduces manual errors

**Alternatives Considered:**
- Manual bindings: Error-prone, hard to maintain
- Custom codegen: Reinventing the wheel

## Tooling

### Required Tools

```bash
# Install Component Model tooling
cargo install wasm-tools
cargo install cargo-component
cargo install wit-bindgen-cli

# Verify installation
wasm-tools --version
cargo component --version
wit-bindgen --version
```

### Build Commands

```bash
# Build extension as component
cd extensions/issues/api
cargo component build --release

# Inspect component
wasm-tools component wit extension.component.wasm

# Validate component
wasm-tools validate extension.component.wasm

# Optimize component
wasm-opt -O3 extension.component.wasm -o extension.optimized.wasm
```

## Testing Strategy

1. **Unit Tests**
   - WIT interface compatibility
   - Resource lifecycle
   - Type conversions

2. **Integration Tests**
   - Load Component Model extensions
   - Execute GraphQL queries
   - Database operations
   - Error handling

3. **Performance Tests**
   - Component instantiation time
   - Function call overhead
   - Memory usage vs Preview 1

4. **Migration Tests**
   - Backward compatibility mode
   - Preview 1 → Component migration
   - Schema compatibility

## Performance Considerations

- **Binary Size**: Component Model adds overhead (~10-20KB per component)
  - Mitigation: wasm-opt, stripping debug info
- **Instantiation**: Component loading slightly slower than modules
  - Mitigation: Cache instantiated components
- **Function Calls**: Similar performance to Preview 1
- **Type Conversions**: Native types faster than JSON serialization

## Open Questions

1. **When to drop Preview 1 support?**
   - After all extensions migrated?
   - Set deprecation timeline?
   - Keep for backward compatibility?

2. **How to handle async in Component Model?**
   - Wait for async component model spec?
   - Use sync-over-async pattern?
   - Future migration to native async?

3. **Should we version the WIT interface?**
   - `forge:extension@0.1.0` → `@0.2.0`?
   - Semantic versioning for interfaces?
   - Backward compatibility guarantees?

4. **How to compose multi-language extensions?**
   - Rust + JS components?
   - Shared resource handles?
   - Cross-language testing?

5. **What about existing deployed extensions?**
   - Require recompilation?
   - Provide migration tool?
   - Support both formats indefinitely?

## Success Criteria

- All extensions migrated to Component Model
- No performance regression (within 5%)
- Binary size increase <20%
- Clear migration documentation
- No breaking changes for extension developers
- Full test coverage maintained

## References

- Component Model spec: https://github.com/WebAssembly/component-model
- WASI Preview 2: https://github.com/WebAssembly/WASI/blob/main/preview2/
- WIT format: https://component-model.bytecodealliance.org/design/wit.html
- Wasmtime Component Model: https://docs.wasmtime.dev/lang-rust/component-model.html
- cargo-component: https://github.com/bytecodealliance/cargo-component

## Future Enhancements

- **Async Component Model**: Native async/await when spec stabilizes
- **Multi-Language Extensions**: Compose Rust + JS + Python components
- **Component Registry**: Publish/discover components
- **Component Composition**: Combine multiple components into one
- **WASI Proposals**: Adopt new WASI interfaces (HTTP, sockets, etc.)
- **Component Linking**: Link components at runtime
