# Forgepoint

A single-user code forge with a GraphQL API for managing groups and repositories, featuring an extensible WASM-based plugin architecture.

## Features

- **GraphQL API** - Complete API for managing repositories and groups
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

### Link a Remote Repository
```graphql
mutation {
  linkRemoteRepository(url: "https://github.com/user/repo") {
    id
    slug
    remoteUrl
  }
}
```

### Clone a Remote Repository
```graphql
mutation {
  cloneRepository(url: "https://github.com/user/repo") {
    id
    slug
    remoteUrl
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

- [Architecture Decision Records](docs/adrs/)
- [Product Requirements](docs/prds/)
- [RFCs](docs/rfcs/)
- [Development Guide](CLAUDE.md)

## License

[License information to be added]

## Contributing

[Contributing guidelines to be added]