# @forgepoint/astro-integration-issues

Astro integration for the Issues extension. Provides UI components and routes for issue tracking functionality.

## Installation

```bash
bun add @forgepoint/astro-integration-issues
```

## Usage

Add the integration to your Astro config:

```javascript
// astro.config.mjs
import { defineConfig } from 'astro/config';
import vue from '@astrojs/vue';
import issuesIntegration from '@forgepoint/astro-integration-issues';

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration(),
  ],
});
```

## Features

- **Issue List Page**: `/issues` - Browse all issues
- **Issue Detail Page**: `/issues/[id]` - View individual issue details
- **Vue Components**: Reusable components for displaying issues
- **Type-Safe Client**: GraphQL client with TypeScript types

## Development

Generate GraphQL types from the running server:

```bash
bun run codegen
```

Watch for changes:

```bash
bun run dev
```

## Requirements

- Forge server running with the Issues extension loaded
- Astro project with Vue integration
- GraphQL endpoint at `http://localhost:8000/graphql`

## Architecture

This integration follows the Extension Package Architecture (PRD-0002):

- **Frontend**: Astro pages + Vue 3 components
- **Backend**: Communicates with the Issues WASM extension via GraphQL
- **Type Safety**: Uses GraphQL Code Generator for type-safe API calls

## Compatible Extension Versions

This integration is compatible with Issues extension `^0.1.0`.
