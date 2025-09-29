# ADR-0001: Forge Database Layout

- Status: Draft
- Date: 2024-05-29
- Authors: Forgepoint Dev Team

## Context
The forge server must capture minimal metadata about groups and repositories while delegating rich configuration to CUE files stored in each repository. We need persistence that supports:
- cuid2 identifiers for groups and repositories.
- Kebab-case slug uniqueness scoped to a parent group (or root).
- Efficient lookup by path when resolving GraphQL queries.
- Isolation between forge-wide metadata and per-repository state.

## Decision
1. Use SQLite (libSQL-compatible) databases located under the directory specified by `FORGE_DB_PATH`.
2. Store global forge metadata in `FORGE_DB_PATH/forge.db` with the following tables:
   - `groups(id TEXT PRIMARY KEY, slug TEXT NOT NULL, parent TEXT NULL, UNIQUE(parent, slug))` where `parent` is a nullable foreign key to `groups(id)`.
   - `repositories(id TEXT PRIMARY KEY, slug TEXT NOT NULL, "group" TEXT NULL, remote_url TEXT NULL, UNIQUE("group", slug))` where `"group"` is a nullable foreign key to `groups(id)`. `remote_url` is populated only for linked remote repositories.
   - Partial index `idx_repositories_remote_url` enforces uniqueness of `remote_url` values when present.
3. Materialize per-repository databases as flat files named `{path-with-dots}.db` within the same directory, e.g. `group.subgroup.repo.db`.
4. Defer migrations and schema management details to a subsequent migration plan once we integrate an ORM or migration tool.

## Consequences
- Lookups by path can be implemented by iteratively resolving slugs against the `groups` table.
- `UNIQUE(parent, slug)` and `UNIQUE("group", slug)` enforce the required slug uniqueness rules without additional application logic.
- Storing per-repository data separately allows us to keep forge metadata small and to evolve repository-level schemas independently.
- Care must be taken to sanitize filenames when generating `{path-with-dots}.db`, but the kebab-case slug constraint limits the character set and keeps naming deterministic.
