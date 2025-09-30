# @forgepoint/astro-integration-issues

Astro integration for the Issues extension. Provides UI components and routes for issue tracking functionality.

## Installation

```bash
bun add @forgepoint/astro-integration-issues
```

## Usage

Add the integration to your Astro config. To enable the repository tab slot, pass the `slotRegistry`:

```javascript
// astro.config.mjs
import { defineConfig } from 'astro/config';
import vue from '@astrojs/vue';
import { createSlotRegistry, createSlotPlugin } from './src/lib/slot-plugin.ts';
import issuesIntegration from '@forgepoint/astro-integration-issues';

const slotRegistry = createSlotRegistry();
const slotPlugin = createSlotPlugin(slotRegistry);

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration({ slotRegistry }), // Enables Issues tab on repository pages
  ],
  vite: {
    plugins: [slotPlugin],
  },
});
```

### Without Slot System

If you only want the standalone `/issues` routes without the repository tab:

```javascript
import issuesIntegration from '@forgepoint/astro-integration-issues';

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration(), // No slotRegistry - only adds routes
  ],
});
```

## Features

- **Issue List Page**: `/issues` - Browse all issues
- **Issue Detail Page**: `/issues/[id]` - View individual issue details
- **Repository Tab Slot**: Adds an "Issues" tab to repository pages (when `slotRegistry` is provided)
- **Vue Components**: Reusable components for displaying issues
- **Type-Safe Client**: GraphQL client with TypeScript types

## Extension Slots

This integration registers the following slots when `slotRegistry` is provided:

### Repository Tab: `issues`

Adds an "Issues" tab to all repository pages, displaying issues specific to that repository.

**Context provided:**
```typescript
{
  id: string;         // Repository ID
  slug: string;       // Repository slug
  fullPath: string;   // Full path (e.g., "group/repo")
  isRemote: boolean;  // Whether it's a remote repository
  remoteUrl: string | null;
}
```

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
