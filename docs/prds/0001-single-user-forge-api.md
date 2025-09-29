# PRD-0001: Single-User Forge API

- Status: Draft
- Date: 2024-05-29
- Authors: Forgepoint Dev Team

## Problem Statement
The forge owner needs a lightweight server that exposes a stable API for managing groups and repositories in a single-user/single-organization environment. The API must enable tooling and UI layers to query the existing structure and provision new nodes without depending on direct database access.

## Goals
- Provide a GraphQL API that lists groups and repositories with minimal metadata.
- Support creation of groups and repositories with slug uniqueness enforcement.
- Resolve repositories and groups by slash-delimited paths.
- Keep configuration 12-factor compliant via environment variables.
- Maintain anonymous access temporarily to unblock development, while leaving space for future ATProto OAuth integration.

## Non-Goals
- Managing rich repository metadata sourced from CUE files.
- Performing Git operations or filesystem scaffolding during repository creation.
- Implementing authentication, authorization, or rate limiting (tracked for future work).

## User Stories
- As the forge owner, I can fetch all groups and repositories so that I can render navigation in my UI.
- As the forge owner, I can create a subgroup to organize related repositories under an existing parent.
- As the forge owner, I can create a repository at the root or within an existing group without colliding with existing slugs.
- As a developer, I can resolve a group or repository by its human-readable path to power CLI tooling.

## Functional Requirements
- Environment variable `FORGE_DB_PATH` points to the directory containing `forge.db` and per-repository databases.
- GraphQL schema exposes `Group` and `Repository` types with fields: `id`, `slug`, and parent references per RFC-0001.
- Queries: `getAllGroups`, `getAllRepositories`, `getGroup(path: String!)`, `getRepository(path: String!)`.
- Mutations: `createGroup(input: { slug, parentId? })`, `createRepository(input: { slug, groupId? })`.
- Mutation: `linkRemoteRepository(url: String!)` derives a slug from the external URL, saves the record as read-only, and prevents duplicate links.
- Slugs must be validated as lowercase kebab-case before persistence.
- Path resolution walks one segment at a time, returning `null` if any segment is missing.
- Validation failures surface as GraphQL errors with `extensions.code = "BAD_USER_INPUT"`.

## Success Metrics
- Server can be configured solely through environment variables with no hard-coded paths.
- All defined queries and mutations return expected data using the SQLite backing store.
- Repository and group creation latency remains under 100 ms on a typical dev machine (local SQLite).

## Open Questions
- How will ATProto OAuth and the owner DID integrate with the GraphQL context?
- Which migration tooling will we adopt to evolve the SQLite schemas?
- What triggers will populate per-repository databases beyond creation time?
