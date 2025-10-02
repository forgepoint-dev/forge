# Forgepoint

A single-tenant code forge with a GraphQL API for managing groups and repositories, featuring an extensible WASM-based plugin architecture. Multiple users can authenticate and collaborate within a single tenant.

## Features

- **GraphQL API** - Complete API for managing repositories and groups
- **HTTP CLI** - Remote repository management via command-line
- **ATProto Authentication** - Multi-user OAuth authentication with Bluesky and other ATProto services
- **Hierarchical Organization** - Nest repositories within groups
- **WASM Extensions** - Secure, sandboxed plugin system
- **Local & Remote Repositories** - Support for both local working copies and remote repository links
- **File Browsing** - Browse repository contents via GraphQL
- **SQLite Storage** - Lightweight, embedded database

## Quick Start

### Prerequisites

- Rust (edition 2024)
- Nix (optional, for development environment)

### Development Setup

Using Nix (recommended):
```bash
nix develop --impure
```

### Running the Server

Development mode with in-memory database:
```bash
cd server
FORGE_IN_MEMORY_DB=true cargo run --bin server
```

Production mode with persistent storage:
```bash
FORGE_DB_PATH=./.forge/db FORGE_REPOS_PATH=./.forge/repos cargo run --bin server
```

The GraphQL playground will be available at http://localhost:8000/graphql

### Using the CLI

The CLI provides HTTP-based remote management of repositories:

```bash
# Build the CLI
cd cli
cargo build --release

# Create a repository
../target/release/forge repo create my-project

# Link a remote repository
../target/release/forge repo link https://github.com/torvalds/linux

# Connect to a remote server
../target/release/forge --api-url https://forge.example.com/graphql repo create my-project
```

See [cli/README.md](cli/README.md) for complete CLI documentation.

## Architecture

- **Backend**: Rust with Axum and async-graphql
- **Database**: SQLite with SQLx
- **Extensions**: WebAssembly modules using Wasmtime
- **Frontend**: Astro + Vue 3 with Tailwind CSS
- **Design System**: Vue 3 components with Storybook

## GraphQL API Examples

### Create a Group
```graphql
mutation {
  createGroup(input: { slug: "my-projects" }) {
    id
    slug
  }
}
```

### Create a Repository
```graphql
mutation {
  createRepository(input: { slug: "my-app", groupId: "grp_..." }) {
    id
    slug
  }
}
```

### Browse Repository Files
```graphql
query {
  getRepository(path: "my-projects/my-app") {
    entries(path: "") {
      name
      type
      size
    }
  }
}
```

## Extension System

Forgepoint supports WebAssembly extensions that can:
- Extend the GraphQL schema
- Handle custom field resolution
- Manage isolated databases
- Run with resource limits for security

Extensions are loaded from `server/extensions/` at startup.

## Architecture

- **Backend**: Rust with GraphQL API and WASM extension system
- **Frontend**: Astro with Vue 3 components and Tailwind CSS
- **Database**: SQLite with isolated per-extension databases
- **Extensions**: WebAssembly modules for secure, sandboxed functionality
- **Development**: Nix flakes for reproducible environments

## Development

### Run Tests
```bash
cd server && cargo test
```

### Lint Code
```bash
cd server && cargo clippy
```

### Format Code
```bash
nix fmt  # Formats all tracked files
```

## Documentation

- [CLI Documentation](cli/README.md)
- [Architecture Decision Records](docs/adrs/)
- [Product Requirements](docs/prds/)
- [RFCs](docs/rfcs/)
- [Development Guide](CLAUDE.md)
- [Authentication Setup](docs/guides/authentication.md)

## Authentication

Forgepoint supports optional ATProto OAuth authentication. To enable:

```bash
export ATPROTO_CLIENT_ID="your-client-id"
export ATPROTO_CLIENT_SECRET="your-client-secret"
export ATPROTO_REDIRECT_URI="http://localhost:8000/auth/callback"
```

See [Authentication Setup Guide](docs/guides/authentication.md) for details.

## License

[License information to be added]

## Contributing

[Contributing guidelines to be added]
