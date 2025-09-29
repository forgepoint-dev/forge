# AI Agents Guide

This document provides guidance for AI agents (including GitHub Copilot, Claude, GPT, etc.) working on the Forge repository.

## Project Context

Forge is a personal/organizational Git forge focused on providing a streamlined development experience for individual developers and teams. Unlike public Git hosting services, Forge centers around your personal repositories and development workflow.

### Key Principles
- **Single-user/organization focus**: The experience is tailored for personal or team use, not public hosting
- **Repository-centric**: Focus on repos, monorepos, and higher-level products
- **Collaboration-aware**: Includes PRs and issues, but they serve the primary user's workflow
- **Development-first**: Optimized for active development rather than project discovery

## Architecture Overview

### Backend (Rust)
- **Location**: `server/`
- **Technology**: Rust with GraphQL API
- **Database**: SQLite (configurable path via `FORGE_DB_PATH`)
- **Storage**: File system for Git repositories (`FORGE_REPOS_PATH`)
- **Build**: Standard Cargo workflows within Nix environment

### Frontend (JavaScript/TypeScript)
- **Location**: `apps/web/`
- **Technology**: Astro + Vue 3 (Composition API)
- **Styling**: Tailwind CSS with shadcn-like design tokens
- **Package Management**: Bun (not npm/yarn)
- **Build**: Astro build system

### Development Environment
- **Nix Flakes**: Reproducible development environment
- **devenv**: Simplified Nix environment management
- **Languages**: Rust (stable), JavaScript/Node.js, Nix
- **Tools**: cargo, bun, nixd, rustfmt, biome, clippy
- **Formatting**: Unified via treefmt (nixfmt, biome, rustfmt)

## Development Workflow

### Environment Setup
1. Install Nix using the Determinate Systems installer
2. Use `nix develop` to enter the development environment
3. All tools (Rust, Bun, formatters) are automatically available

### Common Tasks
- **Start server**: `just server` (creates `.forge/` directories automatically)
- **Frontend development**: `cd apps/web && bun install && bun run dev`
- **Format code**: `nix fmt` (formats all languages)
- **Check Rust**: `cargo check` (from server directory)

### File Organization
- **Documentation**: Use `docs/adrs/`, `docs/prds/`, `docs/rfcs/` with numbered files
- **Components**: Frontend UI components in `apps/web/src/components/ui/`
- **Configuration**: Environment variables, not hardcoded values

## Guidelines for AI Agents

### Code Generation
1. **Follow existing patterns**: Look at current code structure before adding new features
2. **Use environment variables**: Don't hardcode paths or URLs
3. **Respect the architecture**: Backend GraphQL + Frontend Vue/Astro
4. **Match formatting**: Use the project's formatting tools (rustfmt, biome)

### Rust Backend Guidelines
- Use idiomatic Rust patterns
- Leverage the GraphQL schema for API design
- Handle errors gracefully with proper error types
- Use environment variables for configuration
- Follow existing database and file system patterns

### Frontend Guidelines  
- Use Vue 3 Composition API syntax
- Follow Astro patterns for page components
- Use Tailwind classes with existing design tokens
- Manage state appropriately (local vs. global)
- Use Bun for package management commands

### Documentation Guidelines
- Update relevant ADRs when making architectural changes
- Add PRDs for new features with user impact
- Use RFCs for technical proposals requiring discussion
- Follow existing numbering and naming conventions

### Testing Considerations
- Backend: Use Rust's built-in testing framework
- Frontend: Follow Astro/Vue testing best practices
- Integration: Consider the GraphQL API boundary
- Environment: Test within the Nix development environment

## Common Patterns

### Configuration Management
```rust
// Prefer environment variables
let db_path = env::var("FORGE_DB_PATH").unwrap_or_else(|_| "./.forge/db".to_string());
```

### Vue Component Structure
```vue
<!-- Use Composition API -->
<script setup lang="ts">
import { ref } from 'vue'
// Component logic here
</script>

<template>
  <!-- Template with Tailwind classes -->
</template>
```

### Nix Environment Usage
```bash
# Always use nix develop for consistency
nix develop --command cargo build
nix develop --command bun run dev
```

## Error Handling

### Backend Errors
- Use proper Rust error types
- Provide meaningful error messages
- Log errors appropriately for debugging
- Return structured errors in GraphQL responses

### Frontend Errors  
- Handle network errors gracefully
- Provide user-friendly error messages
- Use Vue error boundaries where appropriate
- Log client-side errors for debugging

## Performance Considerations

### Backend
- Efficient database queries (SQLite optimization)
- Proper Git repository handling
- Memory management for long-running server
- GraphQL query optimization

### Frontend
- Astro's static generation capabilities
- Vue component reactivity optimization
- Bundle size management with Bun
- Proper asset loading strategies

## Security Guidelines

- Validate all inputs (GraphQL resolvers, file paths)
- Secure Git repository access patterns
- Proper authentication/authorization patterns
- Environment variable security (no secrets in code)

## Integration Points

### GraphQL API
- Backend exposes GraphQL endpoint
- Frontend consumes via standard GraphQL queries
- Use proper types and schema validation
- Handle real-time updates appropriately

### File System
- Git repositories stored in configurable location
- Database files in configurable location
- Proper permissions and access patterns
- Handle file system errors gracefully

## Getting Help

- **Copilot Instructions**: `.github/copilot-instructions.md`
- **Architecture Decisions**: `docs/adrs/`
- **Development Setup**: Nix flake configuration in `flake.nix`
- **Task Automation**: `Justfile` for common commands

Remember: This is a development-focused Git forge, not a public hosting service. Keep the single-user/organization experience at the center of design decisions.