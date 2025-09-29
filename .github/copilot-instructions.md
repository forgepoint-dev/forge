# GitHub Copilot Instructions

Welcome to the Forge repository! This document provides GitHub Copilot with context about our development setup and project structure.

## Project Overview

Forge is a Git forge designed for single users or organizations, providing a focused experience for managing repositories, monorepos, and higher-level products. The system includes collaboration features (PRs/issues) but centers around your personal or organizational development workflow.

## Repository Structure

```
├── apps/
│   └── web/          # Astro + Vue frontend with Bun package management
├── server/           # Rust backend server
├── docs/
│   ├── adrs/         # Architecture Decision Records
│   ├── prds/         # Product Requirements Documents  
│   └── rfcs/         # Request for Comments
├── design/           # Design assets and documentation
├── flake.nix         # Nix development environment configuration
├── flake.lock        # Nix flake lockfile
└── Justfile          # Task automation (alternative to Makefile)
```

## Development Environment

This project uses **Nix with devenv** for reproducible development environments. The environment is configured in `flake.nix` and includes:

### Languages & Tools
- **Rust**: Stable channel with cargo, clippy, rust-analyzer, rustc, rustfmt, and mold linker
- **JavaScript/Node.js**: With Bun and npm enabled
- **Nix**: With nixd language server

### Code Formatting
- **nixfmt**: For Nix files
- **Biome**: For JavaScript/TypeScript/JSON
- **rustfmt**: For Rust code

## Getting Started

1. **Install Nix** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
   ```

2. **Enter the development environment**:
   ```bash
   nix develop
   ```
   This will automatically install and configure all required tools.

3. **Run the server**:
   ```bash
   just server
   # or with custom paths:
   just server ./custom/db ./custom/repos
   ```

4. **Develop the web frontend**:
   ```bash
   cd apps/web
   bun install
   bun run dev
   ```

## Code Conventions

### Rust (Backend)
- Follow standard Rust conventions with `rustfmt`
- Use `clippy` for linting
- Server binary is in `server/src/main.rs`
- Database path configurable via `FORGE_DB_PATH` environment variable
- Repository storage path configurable via `FORGE_REPOS_PATH`

### JavaScript/TypeScript (Frontend)  
- Uses Astro + Vue architecture
- Styled with Tailwind CSS using shadcn-like tokens
- Package management with Bun
- UI components in `apps/web/src/components/ui/`
- Main landing component: `apps/web/src/components/HomeLanding.vue`

### Documentation
- Architecture decisions go in `docs/adrs/`
- Product requirements in `docs/prds/`
- Technical RFCs in `docs/rfcs/`
- Use numbered files (e.g., `0001-title.md`)

## Common Tasks

Use the `Justfile` for common development tasks:
- `just server` - Run the Forge server with default settings
- `just server <db_path> <repos_path>` - Run server with custom paths

## Environment Variables

- `FORGE_DB_PATH`: SQLite database location (default: `./.forge/db`)
- `FORGE_REPOS_PATH`: Git repositories storage (default: `./.forge/repos`)  
- `PUBLIC_FORGE_GRAPHQL_URL`: GraphQL endpoint for frontend (default: `http://localhost:8000/graphql`)

## Testing & Formatting

Format code across the entire project:
```bash
nix fmt
```

The project uses treefmt for unified formatting across all languages.

## Tips for Copilot

- When working on Rust code, remember this is a web server with GraphQL API
- Frontend uses Vue 3 Composition API within Astro pages
- Configuration is environment-based, prefer environment variables over hardcoded values
- Follow the existing patterns in ADRs and RFCs for architectural decisions
- The project emphasizes single-user/organization focus rather than public Git hosting