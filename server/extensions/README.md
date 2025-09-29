# Extensions Directory

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

## Development

To create an extension:

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
}

enum IssueStatus {
  OPEN
  CLOSED
  IN_PROGRESS
}

extend type Query {
  getAllIssues: [Issue!]!
  getIssue(id: ID!): Issue
}

extend type Mutation {
  createIssue(input: CreateIssueInput!): Issue!
  updateIssue(id: ID!, input: UpdateIssueInput!): Issue
}
```

Note: Extensions should carefully name their fields to avoid conflicts with core schema and other extensions.