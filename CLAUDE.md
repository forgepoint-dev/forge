# Forgepoint Development Context

## Project Overview

Forgepoint is a single-user code forge with a GraphQL API for managing groups and repositories. It provides a lightweight server that exposes a stable API for organizing repositories in a single-user/single-organization environment, with an extensible WASM-based plugin architecture.

## Architecture

- **Backend**: Rust (edition 2024) with Axum web framework and async-graphql for GraphQL
- **Database**: SQLite with async-graphql-axum for query/mutation handling
- **Extensions**: WebAssembly (WASM) runtime using Wasmtime for secure, isolated plugins
- **Frontend**: Astro + Vue 3 with Tailwind CSS (in `apps/web/`)
- **Design System**: Vue 3 components with Storybook (in `design/`)

## Key Technologies

- **Runtime**: tokio async runtime with multi-threading
- **Database**: SQLx for SQLite with WAL journaling
- **Git Operations**: gix library for Git interactions
- **WASM**: wasmtime with component-model for extensions
- **GraphQL**: async-graphql with dynamic schema support
- **Validation**: Custom slug validation (lowercase kebab-case)
- **Testing**: tokio-test, tempfile, wat for WASM tests

## Project Structure

The project follows a modular, feature-based architecture:

- **server/** - Rust backend with GraphQL API and WASM extension system
- **apps/web/** - Astro frontend with Vue 3 components
- **design/** - Reusable Vue 3 component library with Storybook
- **docs/** - Architecture decisions (ADRs), product requirements (PRDs), and RFCs
- **flake.nix** - Nix development environment for reproducible builds

### Backend Organization

The server uses feature-based modules where each domain (groups, repositories, extensions) contains its complete implementation including queries, mutations, models, and business logic. This keeps related code together and makes the system easier to understand and maintain.

## Development Environment

### Setup

The project uses Nix flakes for reproducible development environments:

```bash
nix develop --impure
```

This provides:
- Rust stable toolchain with cargo, clippy, rustfmt, rust-analyzer
- Bun 1.1.30 for JavaScript workspaces
- Mold linker for faster Rust builds

### Environment Variables

For server operation:

- `FORGE_DB_PATH` - Directory containing forge.db and extension databases (required for persistent mode)
- `FORGE_REPOS_PATH` - Root directory for repository working copies (required for persistent mode)
- `FORGE_IN_MEMORY_DB` - Set to "true" for in-memory SQLite (recommended for development and testing)

### Running the Server

**Development Mode (In-Memory Database - Recommended):**

```bash
cd server
FORGE_IN_MEMORY_DB=true cargo run --bin server
```

This mode:
- Uses in-memory SQLite (no persistent storage)
- Creates temporary directories for repositories
- Perfect for development and testing
- Fast startup with no cleanup needed

**Production Mode (Persistent Database):**

```bash
just server                                    # Run with default paths (./.forge/db, ./.forge/repos)
just server /path/to/db /path/to/repos        # Run with custom paths
```

Or directly with cargo:

```bash
cd server
FORGE_DB_PATH=../.forge/db FORGE_REPOS_PATH=../.forge/repos cargo run --bin server
```

## Core Features

### Groups

- Hierarchical organization of repositories
- Slug-based identification (lowercase kebab-case)
- Optional parent relationships for nesting
- GraphQL queries: `getAllGroups`, `getGroup(path: String!)`
- GraphQL mutations: `createGroup(input: { slug, parentId? })`

### Repositories

- Can exist at root or within groups
- Support for local repositories (with working copies)
- Support for remote repositories (read-only, linked by URL)
- File browsing with `entries(path: String)` resolver
- GraphQL queries: `getAllRepositories`, `getRepository(path: String!)`
- GraphQL mutations: `createRepository(input: { slug, groupId? })`, `linkRemoteRepository(url: String!)`

### Extension System (WASM)

Extensions are WebAssembly modules that can:
- Provide GraphQL schema fragments (types, queries, mutations)
- Handle field resolution via WASM function calls
- Manage isolated SQLite databases per-extension
- Run with resource limits (memory, fuel)

See [docs/adrs/0002-wasm-extension-system.md](docs/adrs/0002-wasm-extension-system.md) for details.

Extensions are loaded from `server/extensions/` directory at startup.

## Code Organization

### Feature-Based Modules

The codebase follows feature-based organization:

- `group/` - All group-related code (queries, mutations, models, db access)
- `repository/` - All repository-related code
- `extensions/` - Extension system implementation
- `graphql/` - GraphQL schema building and error handling
- `api/` - HTTP server and middleware
- `validation/` - Shared validation utilities

### Testing Structure

Tests are co-located with implementation:
- Unit tests in `#[cfg(test)] mod tests` blocks
- Integration tests use real SQLite (in-memory mode)
- Extension tests use WAT (WebAssembly Text) format for fixtures

## Testing Guidelines (Per User Rules)

**CRITICAL**: Follow these testing principles strictly:

1. **Test Behavior, Not Implementation** - Tests should validate what the code does, not how it does it
2. **No Mocks** - Use real dependencies (in-memory SQLite, temp files, actual WASM modules)
3. **Real Schemas/Types** - Never redefine types in tests, import from production code
4. **Small, Pure Functions** - Write testable, composable functions
5. **Immutable Values** - Prefer immutable data structures
6. **No Shortcuts** - Think through problems before taking hacky solutions

### Test Feature Flag

Use `#[cfg(feature = "test-support")]` for test-only helpers in production code.

## Extension System Details

### WebAssembly Interface (WIT)

Extensions implement a component model interface that provides:

- **Host Services**: Logging and database access through imported interfaces
- **Extension API**: Core functions for initialization, schema provision, and field resolution
- **Type Safety**: Structured data exchange with proper error handling
- **Resource Management**: Clean initialization and shutdown lifecycle

The interface uses the WASI component model for secure, isolated execution with controlled access to host resources.

### Extension Loading Process

Extensions are discovered and loaded at server startup:

1. **Discovery**: Scan extensions directory for `.wasm` files
2. **Instantiation**: Create Wasmtime engine with WASI support and resource limits
3. **Initialization**: Each extension receives its own SQLite database and configuration
4. **Schema Integration**: Extensions provide GraphQL schema fragments that are merged into the main API
5. **Runtime**: Extensions handle field resolution requests during GraphQL query execution

This process ensures extensions are isolated, secure, and can extend the API without modifying core server code.

### Extension Security

- Sandboxed WASM execution (no direct system access)
- Fuel-based execution limits
- Memory limits enforced by Wasmtime
- WASI filesystem access restricted to extension database

## Common Commands

### Development

```bash
just server                      # Run server with default paths
cd server && cargo test          # Run all tests
cd server && cargo clippy        # Lint checks
cd server && cargo fmt           # Format code
```

### Frontend

```bash
cd apps/web
bun install
bun run dev                     # Start Astro dev server
bun run build                   # Build for production
```

### Design System

```bash
cd design
bun install
bun run storybook              # Start Storybook on :6006
bun run build-storybook        # Build static Storybook
```

## Important Patterns

### Database Operations

- Use SQLx compile-time checked queries
- Handle `Option<T>` for nullable foreign keys
- Normalize paths with `db::normalize_path()`
- Use transactions for multi-step mutations

### GraphQL Resolvers

- Return `async_graphql::Result<T>` from resolvers
- Use `Context<'_>` to access shared state (SqlitePool, RepositoryStorage)
- Map database errors with `graphql::errors::internal_error()`
- Use `graphql::errors::bad_user_input()` for validation failures

### Slug Validation

All user-provided slugs must be validated:

```rust
use crate::validation::slug::validate_slug;

validate_slug(&input.slug)?;  // Returns Result<(), FieldError>
```

Valid slugs: lowercase, alphanumeric, hyphens only, no leading/trailing hyphens.

### Path Resolution

Groups and repositories can be resolved by slash-delimited paths:

```graphql
query {
  getGroup(path: "parent-group/child-group")
  getRepository(path: "group/my-repo")
}
```

Implementation walks path segments one at a time.

### File Operations

- Files always have a blank line at the end
- Use `RepositoryStorage` for repository filesystem operations
- Handle both local and remote repositories appropriately

## GraphQL Playground

Available at `http://localhost:8000/graphql` when server is running.

Example queries:

```graphql
# List all groups
query {
  getAllGroups {
    id
    slug
    parent { slug }
  }
}

# Create a repository
mutation {
  createRepository(input: { slug: "my-project", groupId: "grp_..." }) {
    id
    slug
    group { slug }
  }
}

# Browse repository files
query {
  getRepository(path: "my-group/my-repo") {
    entries(path: "") {
      name
      type
      size
    }
  }
}
```

## Key Constraints

1. **Single-User** - No authentication/authorization implemented yet
2. **Slug Uniqueness** - Within same parent for groups; within same group for repositories
3. **Read-Only Remotes** - Linked remote repositories cannot be modified
4. **SQLite-Based** - All data in SQLite (main forge.db + per-extension databases)
5. **No Git Operations on Create** - Repository creation only creates database records and directories

## Future Considerations

- ATProto OAuth integration
- Database migration tooling
- Hot-reloading of WASM extensions
- Component Model adoption (WASI Preview 2)
- Rich repository metadata from CUE files
- Git operations integration

## Documentation

- **ADRs**: [docs/adrs/](docs/adrs/) - Architecture decisions with context and rationale
- **PRDs**: [docs/prds/](docs/prds/) - Product requirements and goals
- **RFCs**: [docs/rfcs/](docs/rfcs/) - Technical proposals
- **AGENTS.md**: Guidelines for GitHub Copilot coding agents

## Formatting

Use `treefmt` for consistent formatting:

```bash
nix fmt                        # Format all tracked files
```

Configured formatters:
- Rust: rustfmt
- Nix: nixfmt
- JavaScript/TypeScript: Biome

## Code Quality

- All code must pass clippy without warnings
- Tests must pass before commits
- Use `#[allow(dead_code)]` sparingly with justification comments
- Prefer `anyhow::Result` for application errors
- Use `async_graphql::Result` for GraphQL resolvers

## Working with Extensions

### Creating a New Extension

Extensions are Rust projects compiled to WebAssembly that implement the component model interface:

1. Create a Rust project targeting `wasm32-wasip1`
2. Implement the WIT interface for extension functionality
3. Compile to `.wasm` and place in the extensions directory
4. Server discovers and loads extensions automatically on startup

Extensions can provide new GraphQL types, queries, and mutations while maintaining complete isolation from the core server and other extensions.

### Extension Development Tips

- Use `#[tokio::test]` for async tests
- Test extensions with real WASM modules (use WAT format for fixtures)
- Each extension gets its own SQLite database
- Schema fragments must be valid GraphQL SDL
- Field resolvers receive JSON arguments and return JSON results