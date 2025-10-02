# Forge CLI

The Forge CLI is a command-line tool for managing repositories in Forgepoint via HTTP. It communicates with the Forgepoint GraphQL API over the network, making it suitable for remote management.

## Installation

Build the CLI from the cli directory:

```bash
cd cli
cargo build --release
```

The binary will be available at `../target/release/forge`.

Alternatively, build from the workspace root:

```bash
cargo build --release --package forge-cli
```

## Usage

### Configuration

The CLI communicates with the Forgepoint server via HTTP. By default, it connects to `http://localhost:8000/graphql`.

You can specify a different API endpoint using the `--api-url` flag:

```bash
forge --api-url https://forge.example.com/graphql repo create my-project
```

### Commands

#### Create a Repository

Create a new repository on the server:

```bash
forge repo create <slug> [--group <group-id>]
```

Examples:

```bash
# Create a repository at the root level
forge repo create my-project

# Create a repository within a group
forge repo create my-lib --group grp_abc123xyz

# Connect to a remote server
forge --api-url https://forge.example.com/graphql repo create my-project
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
forge repo link https://github.com/torvalds/linux

# Link to a remote server
forge --api-url https://forge.example.com/graphql repo link https://github.com/rust-lang/rust
```

The CLI will automatically extract the repository name from the URL and use it as the slug.

## Architecture

The CLI is designed as a **remote management tool** that works over HTTP:

- **HTTP-Only Communication**: All operations are performed via GraphQL over HTTP
- **No Direct Database Access**: The CLI never touches the database directly
- **Server-Side Logic**: All validation and business logic happens on the server
- **Lightweight Client**: The CLI is a thin client that just formats requests and displays responses

### Key Benefits

1. **Remote Management**: Manage repositories on any Forgepoint server from anywhere
2. **No Database Dependencies**: No need for SQLite or database configuration
3. **Consistent Behavior**: Uses the same GraphQL API as the web interface
4. **Easy Distribution**: Simple binary with minimal dependencies

## Development

### Running from Source

You can run the CLI directly with cargo:

```bash
cd cli
cargo run -- repo create my-test
```

### Testing

To test the CLI, you need a running Forgepoint server:

1. Start the server:
```bash
cd server
FORGE_IN_MEMORY_DB=true cargo run --bin server
```

2. In another terminal, test the CLI:
```bash
cd cli

# Test repository creation
cargo run -- repo create test-repo

# Test remote linking
cargo run -- repo link https://github.com/example/repo

# Test error handling
cargo run -- repo create Invalid_Slug
```

## Examples

### Basic Usage

```bash
# Create a repository
forge repo create my-project
✓ Repository created successfully!
  ID:   a7ckzfmrwjm9p1ldkaift03j
  Slug: my-project
```

### With Remote Server

```bash
# Create a repository on a remote server
forge --api-url https://forge.example.com/graphql repo create production-app
✓ Repository created successfully!
  ID:   fmwdhm5q08nh85l9o6395w0j
  Slug: production-app
  Group: infrastructure (grp_abc123)
```

### Error Handling

The CLI provides clear, user-friendly error messages:

```bash
$ forge repo create Invalid_Slug
GraphQL errors: slug must be lowercase kebab-case
```

## Comparison with Previous Implementation

The original CLI implementation (now removed) was a **local tool** that:
- Connected directly to the SQLite database
- Required database and repository paths to be configured
- Only worked on the same machine as the server

The new HTTP-based CLI is a **remote tool** that:
- Connects to the GraphQL API over HTTP
- Works from any machine with network access to the server
- Has no dependencies on database or filesystem paths

This architecture better aligns with the product vision of Forgepoint as a **forge** that can be managed remotely.

## Future Enhancements

Potential future additions to the CLI:

- Group management (`forge group create`, `forge group list`)
- Repository listing and search (`forge repo list`, `forge repo search`)
- Repository information (`forge repo info <slug>`)
- Configuration file support (`~/.forgerc`)
- Interactive mode with prompts
- Batch operations from CSV or JSON files
- Authentication support (when server adds auth)
