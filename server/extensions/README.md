# Extensions Directory (DEPRECATED)

**⚠️ DEPRECATION NOTICE:** This directory is deprecated as of 2025-09-30. New extensions should live under `extensions/<extension-name>/{api,ui,shared}` following the Extension Package Architecture (PRD-0002).

**For new extension development, see:**
- [docs/guides/creating-extensions.md](../../docs/guides/creating-extensions.md)
- [docs/prds/0002-extension-packages.md](../../docs/prds/0002-extension-packages.md)
- [extensions/issues/](../../extensions/issues/) - Reference implementation

---

This directory contains WebAssembly (WASM) extension modules for the Forge GraphQL server.

## Extension System Overview

The extension system allows adding new functionality to the GraphQL API without modifying the core server code. Extensions are compiled as WASM modules and provide:

- GraphQL schema fragments (types, queries, mutations) expressed as structured data
- Field resolvers for their schema
- Database management with dedicated SQLite databases
- Secure, sandboxed execution environment

## Extension Structure

Each extension must implement the WIT interface defined in `wit/extension.wit`:

```wit
interface extension {
  record extension-config {
    name: string,
    db-path: string,
    config: option<string>,
    api-version: string,
    capabilities: list<string>,
  }

  enum type-modifier { list-type, non-null }
  record type-ref { root: string, modifiers: list<type-modifier> }
  record field-definition { name: string, description: option<string>, ty: type-ref, args: list<input-value-definition> }
  record object-type { name: string, description: option<string>, interfaces: list<string>, fields: list<field-definition>, is-extension: bool }
  variant schema-type { object-type(object-type), enum-type(enum-type), input-object-type(input-object-type), interface-type(interface-type), scalar-type(scalar-type), union-type(union-type) }
  record schema-fragment { types: list<schema-type> }

  /// Provide API handshake
  get-api-info: func() -> api-info;

  /// Initialize extension with configuration
  init: func(config: extension-config) -> result<_, string>;

  /// Return GraphQL schema as structured data
  get-schema: func() -> schema-fragment;

  /// Run database migrations
  migrate: func(db-path: string) -> result<_, string>;

  /// Handle GraphQL field resolution
  resolve-field: func(field: string, args: string) -> result<string, string>;
}
```

## Loading Process

1. Server scans this directory for `.wasm` files at startup
2. Each WASM module is loaded with WASI support
3. Extension databases are created/opened at `<FORGE_DB_PATH>/<name>.extension.db`
4. Extension is initialized with configuration
5. Database migrations are run
6. GraphQL schema fragment is retrieved and combined with the core schema registry
7. Field resolutions are routed to appropriate extensions

## Migration Path

**Existing extensions in this directory will continue to work** but should be migrated to the new structure:

1. Move extension source to `extensions/<extension-name>/api`
2. Update build configuration to use `justfile` pattern
3. Create corresponding Astro integration in `extensions/<extension-name>/ui`
4. Configure OCI distribution (optional but recommended)

See [docs/guides/creating-extensions.md](../../docs/guides/creating-extensions.md) for complete migration guide.

## Development (Legacy)

⚠️ **For new extensions, use the `extensions/<extension-name>/{api,ui,shared}` structure instead.**

To create a legacy extension:

1. Write extension code in any WASM-compatible language (Rust, AssemblyScript, etc.)
2. Implement the extension interface
3. Compile to WASM
4. Place the `.wasm` file in this directory
5. Restart the server

## Example

A sample "issues" extension might provide:

```graphql
type Issue {
  id: ID!
  title: String!
  description: String
  status: IssueStatus!
  createdAt: String!
  repositoryId: ID!
}

enum IssueStatus {
  OPEN
  CLOSED
  IN_PROGRESS
}

extend type Query {
  getIssuesForRepository(repositoryId: ID!): [Issue!]!
  getIssue(repositoryId: ID!, id: ID!): Issue
}

extend type Mutation {
  createIssue(repositoryId: ID!, input: CreateIssueInput!): Issue!
  updateIssue(repositoryId: ID!, id: ID!, input: UpdateIssueInput!): Issue
}
```

Note: Extensions should carefully name their fields to avoid conflicts with core schema and other extensions.
