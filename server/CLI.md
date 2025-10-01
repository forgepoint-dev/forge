# Forge CLI

The Forge CLI provides command-line tools for managing repositories in Forgepoint.

## Installation

Build the CLI from the server directory:

```bash
cd server
cargo build --release --bin forge
```

The binary will be available at `../target/release/forge`.

## Usage

### Configuration

The CLI uses the same environment variables as the server:

- `FORGE_DB_PATH` - Directory containing forge.db (required for persistent mode)
- `FORGE_REPOS_PATH` - Root directory for repository working copies (required for persistent mode)
- `FORGE_IN_MEMORY_DB` - Set to "true" for in-memory SQLite (useful for testing)

### Commands

#### Create a Repository

Create a new local repository:

```bash
forge repo create <slug> [--group <group-id>]
```

Examples:

```bash
# Create a repository at the root level
FORGE_DB_PATH=./.forge/db FORGE_REPOS_PATH=./.forge/repos \
  forge repo create my-project

# Create a repository within a group
FORGE_DB_PATH=./.forge/db FORGE_REPOS_PATH=./.forge/repos \
  forge repo create my-lib --group grp_abc123xyz

# Create with in-memory database (for testing)
FORGE_IN_MEMORY_DB=true forge repo create test-repo
```

**Note**: Repository slugs must be lowercase kebab-case (alphanumeric and hyphens only, no leading/trailing hyphens).

#### Link a Remote Repository

Link an existing remote repository (read-only):

```bash
forge repo link <url>
```

Examples:

```bash
# Link a GitHub repository
FORGE_DB_PATH=./.forge/db FORGE_REPOS_PATH=./.forge/repos \
  forge repo link https://github.com/torvalds/linux

# Link with in-memory database (for testing)
FORGE_IN_MEMORY_DB=true forge repo link https://github.com/rust-lang/rust
```

The CLI will automatically extract the repository name from the URL and use it as the slug.

## Development

### Running from Source

You can run the CLI directly with cargo:

```bash
cd server
FORGE_IN_MEMORY_DB=true cargo run --bin forge -- repo create my-test
```

### Testing

The CLI reuses the same mutation functions as the GraphQL API, so it's thoroughly tested through the existing test suite. For manual testing:

```bash
# Test repository creation
FORGE_IN_MEMORY_DB=true cargo run --bin forge -- repo create test-repo

# Test remote linking
FORGE_IN_MEMORY_DB=true cargo run --bin forge -- repo link https://github.com/example/repo

# Test error handling
FORGE_IN_MEMORY_DB=true cargo run --bin forge -- repo create Invalid_Slug
```

## Architecture

The CLI is a thin wrapper around the existing repository mutation functions in `server/src/repository/mutations.rs`. This ensures consistency between the CLI and GraphQL API - they use exactly the same validation, database operations, and business logic.

Key features:

- **Shared Logic**: Uses `create_repository_raw()` and `link_remote_repository_raw()` from the repository module
- **Proper Validation**: All slug and URL validation is consistent with the GraphQL API
- **Error Handling**: Clear error messages for common issues (invalid slugs, duplicate repositories, etc.)
- **Database Support**: Works with both in-memory and persistent SQLite databases
- **Directory Creation**: Automatically creates repository directories in the configured location

## Future Enhancements

Potential future additions to the CLI:

- Group management (`forge group create`, `forge group list`)
- Repository listing (`forge repo list`)
- Repository deletion (`forge repo delete`)
- Repository information (`forge repo info <slug>`)
- Configuration file support (`.forgerc`)
- Interactive mode for creating repositories with prompts
- Batch operations from CSV or JSON files
