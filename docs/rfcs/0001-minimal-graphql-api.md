# RFC-0001: Minimal GraphQL Forge API

- Status: Draft
- Date: 2024-05-29
- Authors: Forgepoint Dev Team

## Summary
Define the initial GraphQL API surface for the single-user/organization forge server. Focus on read access to groups and repositories, along with mutations to create these entities, while deferring configuration details sourced from repository CUE files to later iterations.

## Motivation
We need a consistent API layer to support the forge owner in organizing repositories into nested groups and creating new repositories through the server. Establishing the baseline schema and behaviour now allows client and UI workstreams to begin while we continue fleshing out repository-level configuration and authentication features.

## Terminology
- **Group**: Container that can hold repositories and other groups (one level deep in query responses for now).
- **Repository**: Git repository managed by the forge server. Each repository belongs to at most one group.
- **Path**: Slash-delimited string of slugs describing a specific group or repository location, e.g. `parent/child` or `parent/repo`.

## Configuration
- The process reads a `FORGE_DB_PATH` environment variable that points to the directory containing the global `forge.db` and all repository-level SQLite databases.
- The forge owner DID will be provided via a separate environment variable (name TBD in follow-up RFC once authentication is specified).

## Schema Overview
### Types
- `Group`
  - `id: ID!`
  - `slug: String!`
  - `parent: Group` (nullable; root groups return `null`).
  - `repositories: [Repository!]!` (returns direct child repositories; the `group` field on each item resolves to `null` when accessed through this list).
- `Repository`
  - `id: ID!`
  - `slug: String!`
  - `group: Group` (nullable; repositories at the root return `null`).
  - `isRemote: Boolean!` (true when the entry was linked from an external source).
  - `remoteUrl: String` (present only for remote repositories).

### Queries
- `getAllGroups: [Group!]!`
  - Returns all groups, each with its direct parent (if any).
- `getAllRepositories: [Repository!]!`
  - Returns all repositories, each with its immediate parent group (if any).
- `getGroup(path: String!): Group`
  - Resolves slash-delimited paths by walking one segment at a time. Includes the group’s repositories (with `group` resolver suppressed as noted above) and the group’s direct parent.
- `getRepository(path: String!): Repository`
  - Resolves slash-delimited paths to a repository. Only the repository object is returned (no nested group reference in this query).

### Mutations
- `createGroup(input: CreateGroupInput!): Group!`
  - `CreateGroupInput { slug: String!, parentId: ID }`
  - Generates a cuid2 ID server-side and persists the group. Slug uniqueness is enforced per parent (`parentId` + `slug`).
- `createRepository(input: CreateRepositoryInput!): Repository!`
  - `CreateRepositoryInput { slug: String!, groupId: ID }` (`groupId` nullable to allow root-level repositories).
  - Generates a cuid2 ID server-side and persists the repository. Slug uniqueness is enforced per containing group (`groupId` + `slug`).
- `linkRemoteRepository(url: String!): Repository!`
  - Normalises the remote URL, derives a kebab-case slug from the repository name, and stores the record as read-only metadata (root-level for now). Rejects duplicate URLs or slug conflicts at the root.

### Error Handling
- Mutations raise GraphQL errors on validation failures with `extensions.code = "BAD_USER_INPUT"`.
  - Conflicting slugs or invalid IDs return a descriptive message referencing the offending slug or ID.
  - Additional server/runtime errors bubble up with appropriate `extensions.code` values (e.g., `INTERNAL_SERVER_ERROR`).

## Access Control
- The API currently allows anonymous access to all operations to keep development friction low.
- A follow-up RFC will introduce ATProto OAuth enforcement around the configured owner DID; this document should be referenced when aligning access control with the schema defined here.

## Deferred Topics
- Surfacing metadata sourced from in-repository CUE definitions.
- Exposing deeper subgroup hierarchies in a single response.
- Automatic repository scaffolding or Git operations during creation.
- Rate limiting and auditing.
