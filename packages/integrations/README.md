# Integrations

This directory contains Astro integration packages for Forge extensions.

Each integration is an npm package that provides frontend UI components,
routes, and GraphQL client code for a corresponding WASM extension.

## Structure

```
packages/integrations/
└── issues/                    # Example: Issues integration
    ├── package.json          # npm package configuration
    ├── codegen.ts            # GraphQL code generation config
    ├── src/
    │   ├── index.ts          # Astro integration entry point
    │   ├── components/       # Vue components
    │   ├── pages/            # Astro pages
    │   └── lib/
    │       ├── client.ts     # GraphQL client wrapper
    │       └── generated/    # Generated TypeScript types
    └── tsconfig.json
```

## Using Integrations

Integrations are installed as npm packages and added to Astro configuration:

```javascript
// apps/web/astro.config.mjs
import issuesIntegration from '@forgepoint/astro-integration-issues';

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration(),
  ],
});
```

## Development

Each integration uses GraphQL Code Generator to create type-safe clients:

```bash
cd packages/integrations/issues
bun run codegen  # Generate types from GraphQL schema
bun run dev      # Development mode
```

See [PRD-0002](../../docs/prds/0002-extension-packages.md) for architecture details.
