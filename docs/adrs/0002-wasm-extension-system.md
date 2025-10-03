# ADR-0002: WASM Extension System for GraphQL API

- Status: Draft
- Date: 2025-09-29
- Authors: Forgepoint Dev Team

## Context

The forge server needs an extensible architecture that allows adding new functionality without modifying the core codebase. Extensions should be able to:
- Add new GraphQL types and resolvers
- Manage their own data persistence
- Run in a secure, isolated environment
- Be loaded dynamically at runtime

Example use case: An "issues" extension that adds issue tracking capabilities to the GraphQL API with its own data model and storage.

## Decision

### 1. Use WebAssembly (WASM) for Extensions

Adopt WASM as the extension runtime using Wasmtime for the following reasons:
- **Security**: Extensions run in sandboxed environments with no direct system access
- **Language flexibility**: Extensions can be written in any language that compiles to WASM
- **Performance**: Near-native execution speed with JIT compilation
- **Isolation**: Memory and resource isolation between extensions

### 2. Extension Architecture

Each extension will:
- Be compiled as a WASM module (`.wasm` file)
- Have its own SQLite database (`forge_ext_{name}.db`)
- Provide GraphQL schema fragments via WebAssembly Interface Types (WIT)
- Handle its own database migrations internally

### 3. WebAssembly Interface Types (WIT)

Define a standard interface for extensions:

```wit
interface extension {
    // Initialize extension with configuration
    init: func(config: extension-config) -> result<unit, string>

    // Return GraphQL schema SDL as string
    get-schema: func() -> string

    // Run database migrations
    migrate: func(db-path: string) -> result<unit, string>

    // Handle GraphQL field resolution
    resolve-field: func(field: string, args: string) -> result<string, string>
}

record extension-config {
    name: string,
    db-path: string,
    config: option<string>
}
```

### 4. Database Management

- Each extension gets a dedicated SQLite database file
- Database path passed to extension during initialization
- Extensions use WASI filesystem APIs for database access
- Main server manages database file lifecycle (creation, deletion)

### 5. GraphQL Schema Extension

Leverage async-graphql's dynamic schema capabilities:
- Extensions provide SDL strings via `get-schema()`
- Server parses and validates extension schemas
- Schemas merged using namespace prefixing to avoid conflicts
- Field resolutions routed to appropriate extensions

### 6. Extension Loading Process

1. Scan `crates/server/extensions/` directory for `.wasm` files at startup
2. Initialize Wasmtime engine with WASI support
3. For each extension:
   - Load WASM module
   - Create/open extension database
   - Call `init()` with configuration
   - Call `migrate()` for database setup
   - Retrieve schema via `get-schema()`
   - Register schema in GraphQL runtime

### 7. Inter-process Communication

- Use JSON for data serialization between host and WASM
- GraphQL requests/responses marshalled as JSON strings
- Consider MessagePack or Protocol Buffers for performance optimization later

## Implementation Structure

```
crates/
└── server/
    ├── src/
    │   ├── extensions/
    │   │   ├── mod.rs         # Extension system core
    │   │   ├── loader.rs      # WASM loading and lifecycle
    │   │   ├── interface.rs   # WIT bindings and communication
    │   │   └── schema/        # Dynamic schema management modules
    ├── extensions/            # Extension WASM modules
    │   └── issues.wasm
packages/
└── wit/
    └── extension.wit          # WebAssembly Interface Types shared across extensions
```

## Consequences

### Positive

- **Extensibility**: New features can be added without modifying core code
- **Security**: Extensions cannot access host system directly
- **Isolation**: Extension failures don't crash the main server
- **Developer Experience**: Extensions can be written in various languages
- **Modularity**: Features can be enabled/disabled by adding/removing WASM files

### Negative

- **Complexity**: Adds WASM runtime and marshalling overhead
- **Performance**: JSON serialization between host and WASM has overhead
- **SQLite Limitations**: WASI filesystem support for SQLite is read-heavy; write operations may have limitations
- **Debugging**: More challenging to debug across WASM boundary
- **Binary Size**: Wasmtime runtime adds ~10MB to server binary

### Mitigations

- **Performance**: Cache compiled WASM modules and GraphQL schemas
- **SQLite Issues**: Consider alternative approaches like:
  - Host-managed database connections passed to extensions
  - RPC-style database access through host functions
  - Key-value stores instead of full SQL for extensions
- **Debugging**: Implement comprehensive logging and error boundaries

## Alternatives Considered

1. **Dynamic Linking (`.so`/`.dll`)**: Rejected due to security concerns and platform dependencies
2. **Embedded Scripting (Lua/JavaScript)**: Less isolation and security than WASM
3. **Separate Processes**: Higher overhead and complex IPC
4. **Compile-time Plugins**: Requires recompilation, not truly dynamic

## Future Considerations

- WebAssembly Component Model adoption when stabilized
- WASI Preview 2 for improved I/O capabilities
- Shared memory for performance-critical extensions
- Extension marketplace and versioning system
- Hot-reloading of extensions in development

## References

- [Wasmtime Documentation](https://docs.wasmtime.dev/)
- [WASI Specification](https://wasi.dev/)
- [async-graphql Dynamic Schemas](https://docs.rs/async-graphql/latest/async_graphql/dynamic/)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
