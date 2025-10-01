# PRD-0002: Extension Package Architecture

- Status: Accepted
- Date: 2025-09-30
- Last Updated: 2025-09-30
- Authors: Forgepoint Dev Team

## Problem Statement

The forge currently has extension support via WASM modules loaded from a directory or OCI registry (ADR-0003). However, extensions only extend the backend GraphQL API. To provide complete feature packages, we need a way for extensions to also extend the frontend (Astro website) with UI components, routes, and client-side functionality.

The current issues extension is embedded in `server/extensions/example-rust-extension/`, making it difficult to:
- Package and distribute as a cohesive feature
- Version the backend and frontend components together
- Reuse across different forge deployments
- Test in isolation
- Publish to registries (OCI for WASM, npm for frontend)

## Goals

1. **Unified Extension Distribution**: Package backend (WASM) and frontend (Astro integration) together as a logical feature unit.
2. **Independent Distribution Channels**: Publish WASM to OCI registry, Astro integration to npm.
3. **Developer Experience**: Clear patterns and strong typing for creating new extensions with both backend and frontend components.
4. **Monorepo Structure**: Organize extension packages within the forge monorepo for easy development.
5. **Composability**: Allow forge deployments to pick and choose which extensions to install.

## Non-Goals

- Cross-extension dependencies or dependency resolution (per ADR-0003).
- Extension marketplace UI (tracked for future work).
- Hot-reloading of extensions (tracked for future work).

## High-Level Architecture

This diagram illustrates how the frontend and backend components of an extension work together.

```mermaid
graph LR
    subgraph "User's Browser"
        A[UI Components <br> (Vue, from Integration)]
    end

    subgraph "Forge Deployment"
        subgraph "Astro Frontend"
            B[Astro App <br> (apps/web)]
            C[Astro Integration <br> (@forgepoint/astro-integration-*)]
            B -- "Installs" --> C
            A -- "Part of" --> C
        end

        subgraph "Forge Backend"
            D[Forge Server]
            E[WASM Extension <br> (from OCI)]
            D -- "Loads" --> E
        end
    end

    A -- "GraphQL API Call" --> D
    D -- "Resolves via" --> E
    E -- "Accesses" --> F[(Extension DB)]

    style C fill:#cce,stroke:#333
    style E fill:#cfc,stroke:#333
```

## User Stories

### Extension Developer
- As an extension developer, I can create a new extension package with both backend and frontend components using a clear template structure.
- As an extension developer, I can generate type-safe clients for my extension's GraphQL API.
- As an extension developer, I can test my extension in isolation with mock data.
- As an extension developer, I can publish my extension to OCI and npm registries with a single command.

### Forge Administrator
- As a forge admin, I can install an extension by configuring the OCI reference in `forge.ron` and adding the npm package to my Astro project.
- As a forge admin, I can see which extensions are installed and their versions.
- As a forge admin, I can upgrade extensions independently.

### End User
- As a forge user, I can access extension features through a native-looking UI integrated into the forge website.
- As a forge user, extension features feel cohesive with core forge functionality.

## Functional Requirements

### Package Structure

```
packages/
├── wit/
│   └── extension.wit              # Shared WIT interface for all extensions
└── extensions/
    └── issues/
        ├── api/
        │   ├── Cargo.toml         # name = "forgepoint-extension-issues"
        │   ├── src/
        │   │   └── lib.rs         # WASM extension implementation
        │   └── justfile           # Build/test/publish commands
        ├── shared/
        │   └── schema.graphql     # Canonical GraphQL schema fragment
        └── ui/
            ├── package.json       # @forgepoint/astro-integration-issues
            ├── codegen.ts         # GraphQL codegen configuration
            ├── src/
            │   ├── index.ts       # Astro integration entry point
            │   ├── components/    # Vue components for issues UI
            │   ├── pages/         # Astro pages for routes
            │   └── lib/           # GraphQL client and generated types
            └── tsconfig.json
```

### WASM Extension (Backend)

- **Location**: `extensions/issues/api/`
- **Capabilities**: Implements the WIT interface, provides a GraphQL schema fragment, resolves fields, and manages its own data persistence.
- **Build**: Built to `issues.wasm` via `cargo build --target wasm32-wasip1 --release`.
- **Distribution**: Published to an OCI registry (e.g., `ghcr.io/forgepoint-dev/extensions/issues:v1.0.0`).

**Configuration** (in `forge.ron`):
```ron
Config(
    extensions: Extensions(
        oci: [
            OciExtension(
                name: "issues",
                registry: "ghcr.io",
                image: "forgepoint-dev/extensions/issues",
                reference: Tag("v1.0.0"),
            ),
        ],
    ),
)
```

### Astro Integration (Frontend)

- **Location**: `extensions/issues/ui/`
- **Capabilities**: An Astro integration that injects routes, registers UI components, and provides a type-safe GraphQL client for the extension's API.
- **Distribution**: Published to npm as `@forgepoint/astro-integration-issues`.

**Usage** (in `apps/web/astro.config.mjs`):
```javascript
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

### GraphQL Code Generation

To ensure type safety and a superior developer experience, each integration package will use `graphql-codegen`.

- A `codegen.ts` file in the integration's root will configure the code generation process.
- It will introspect the live development server's GraphQL endpoint, which includes the loaded extension's schema.
- The process generates a fully-typed SDK based on the extension's specific queries and mutations.

**Example `package.json` script:**
```json
"scripts": {
  "codegen": "graphql-codegen -c codegen.ts"
}
```

### GraphQL Client Integration

The generated SDK provides type-safe functions for interacting with the API, eliminating manual query construction and type definitions.

```typescript
// extensions/issues/ui/src/lib/client.ts
import { getSdk } from './generated/graphql';
import { graphqlRequest } from 'forge-web/lib/graphql'; // Generic request function from core app

// Adapter function to make the generic `graphqlRequest` compatible with the generated SDK.
const client = async <TData, TVariables>(
  query: string,
  variables?: TVariables
): Promise<TData> => {
  // The `graphqlRequest` function is assumed to handle endpoint, credentials, etc.
  return graphqlRequest<TData>({ query, variables });
};

// Export a pre-configured, type-safe SDK.
export const sdk = getSdk(client);
```

### Vue Components

Components use the generated SDK to fetch data, resulting in cleaner, more maintainable code.

```vue
<!-- extensions/issues/ui/src/components/IssueList.vue -->
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { sdk } from '../lib/client';
import type { GetAllIssuesQuery } from '../lib/generated/graphql';

const issues = ref<GetAllIssuesQuery['getAllIssues']>([]);
const loading = ref(true);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    const response = await sdk.getAllIssues();
    issues.value = response.getAllIssues;
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load issues';
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="issues-list">
    <div v-if="loading">Loading...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <ul v-else>
      <li v-for="issue in issues" :key="issue.id">
        <a :href="`/issues/${issue.id}`">{{ issue.title }}</a>
      </li>
    </ul>
  </div>
</template>
```

### Monorepo Configuration Updates

**`package.json`** (root):
```json
{
  "workspaces": [
    "apps/*",
    "extensions/*/ui"
  ]
}
```

**`flake.nix`** updates:
```nix
{
  # The standard Rust toolchain provided by flake-parts is sufficient
  # for building wasm32-wasip1 targets. No extra packages like wasm-pack
  # are needed for the backend extensions.
  devShells.default = pkgs.mkShell {
    packages = with pkgs; [
      # ... existing packages ...
      # e.g., rust-bin.stable.latest.default
    ];
  };
}
```

## Technical Design

### Request Data Flow

1.  **User Action**: A user navigates to a page provided by an Astro integration (e.g., `/issues`).
2.  **Component Render**: The Astro page renders a Vue component (`IssueList.vue`).
3.  **API Call**: The Vue component calls a method from the type-safe SDK (`sdk.getAllIssues()`).
4.  **HTTP Request**: The SDK uses the core `graphqlRequest` helper to send a GraphQL query to the Forge server.
5.  **Server Routing**: The Forge server's GraphQL gateway receives the query.
6.  **Extension Resolution**: The gateway identifies that the `getAllIssues` query is handled by the `issues` extension and delegates the request to the corresponding WASM module.
7.  **Data Fetching**: The WASM extension executes its logic, fetches data from its database, and returns the result.
8.  **Response**: The response travels back through the gateway to the frontend, where the Vue component updates with the fetched data.

### Extension Slot System

The slot system allows integrations to inject UI components into existing core pages, enabling extensions to seamlessly integrate with the core UI rather than just adding standalone routes.

**Supported Slot Types:**

- **Repository Tabs** (`repo-tabs`): Add tabs to repository pages (e.g., Issues, Pull Requests)
- **Group Tabs** (`group-tabs`): Add tabs to group pages
- **Homepage Widgets** (`homepage-widgets`): Add widgets to the forge homepage

**Implementation:**

The slot system uses Vite virtual modules to aggregate slot registrations from all integrations at build time:

```typescript
// apps/web/src/lib/slot-plugin.ts
export function createSlotRegistry() {
  return {
    repoTabs: [],
    groupTabs: [],
    homepageWidgets: [],
  };
}
```

**Integration Usage:**

Integrations register slots by receiving the `slotRegistry` as an option:

```typescript
// extensions/issues/ui/src/index.ts
export default function issuesIntegration(options) {
  return {
    name: '@forgepoint/astro-integration-issues',
    hooks: {
      'astro:config:setup': ({ injectRoute }) => {
        // Inject standalone routes
        injectRoute({ pattern: '/issues', entrypoint: '...' });
        
        // Register repository tab slot
        if (options?.slotRegistry) {
          options.slotRegistry.repoTabs.push({
            id: 'issues',
            label: 'Issues',
            componentPath: '@forgepoint/astro-integration-issues/components/IssuesTab.vue',
            order: 10,
          });
        }
      },
    },
  };
}
```

**Configuration Example:**

```javascript
// apps/web/astro.config.mjs
import { createSlotRegistry, createSlotPlugin } from './src/lib/slot-plugin.ts';
import issuesIntegration from '@forgepoint/astro-integration-issues';

const slotRegistry = createSlotRegistry();
const slotPlugin = createSlotPlugin(slotRegistry);

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration({ slotRegistry }), // Pass registry to integration
  ],
  vite: {
    plugins: [slotPlugin],
  },
});
```

**Slot Components:**

Slot components receive context data as props, allowing them to render content specific to the current page:

```vue
<!-- IssuesTab.vue -->
<script setup lang="ts">
const props = defineProps<{
  repository: {
    id: string;
    slug: string;
    fullPath: string;
    isRemote: boolean;
    remoteUrl: string | null;
  };
}>();

// Load issues for this specific repository
const issues = await loadIssues(props.repository.id);
</script>
```

**Rendering:**

Core pages use the `ExtensionTabs` component to render slots alongside built-in tabs:

```vue
<ExtensionTabs :repository="repositoryContext">
  <template #files>
    <!-- Built-in files tab content -->
  </template>
</ExtensionTabs>
```

This approach provides:
- **Type Safety**: TypeScript interfaces for context data
- **Decoupling**: Extensions register slots without modifying core code
- **Ordering**: Extensions can control tab order via the `order` property
- **Hot Module Replacement**: Changes to slot components trigger HMR in development

### Version Management

Backend (WASM) and frontend (Astro integration) packages are versioned and published independently to their respective registries (OCI and npm). This allows for decoupled releases.

To manage compatibility, the frontend package will declare which backend versions it supports in its `package.json`:
```json
{
  "name": "@forgepoint/astro-integration-issues",
  "version": "1.1.5",
  "forgepoint": {
    "extensionName": "issues",
    "compatibleExtensionVersions": "^1.2.0"
  }
}
```

### Security Considerations

1.  **WASM Sandboxing**: Extensions run in a Wasmtime sandbox with resource limits (ADR-0002).
2.  **GraphQL Schema Validation**: The server validates extension schemas before merging.
3.  **Frontend Input Validation**: Components must validate user input before sending mutations.
4.  **Authentication**: The GraphQL gateway is responsible for enforcing auth; extensions operate on the assumption of a valid user context.

### Testing Strategy

- **WASM Extension**: Unit tests in Rust for resolver logic, using an in-memory database.
- **Astro Integration**: Vitest for unit testing the integration's hooks and component logic.
- **E2E**: Playwright tests in `apps/web` to verify the complete user flow, from UI interaction to data persistence.

## Success Metrics

1.  **Developer Experience**: Time to create a new "hello world" extension is under 30 minutes.
2.  **Performance**: Extension resolver latency remains under 50ms; page load for extension pages is under 1s.
3.  **Quality**: 100% of extension APIs have TypeScript types via codegen. 90%+ test coverage for extension logic.

## Open Questions

1.  **Component Sharing**: Should extensions share Vue components from the `design/` package, or bundle their own?
    - **Recommendation**: Import from `design` to maintain consistent UI.
2.  **Extension Discovery**: How do admins discover available extensions?
    - **Future work**: An extension marketplace or registry UI.
3.  **Migration Strategy**: How do we migrate the current issues extension?
    - **Approach**: Create the new package structure, copy over the existing code, and archive the old location (`server/extensions/example-rust-extension`).

## Implementation Plan

### Implementation Status: ✅ **COMPLETE** (as of 2025-10-01)

All phases have been implemented and tested. The extension system is functional with the issues extension serving as a reference implementation.

### Phase 1: Infrastructure ✅ COMPLETE
- [x] Create `packages/` directory structure.
- [x] Move `server/wit/extension.wit` to `packages/wit/`.
- [x] Update monorepo configuration (`package.json`, `flake.nix`).

### Phase 2: WASM Extension Migration ✅ COMPLETE
- [x] Move `server/extensions/example-rust-extension` to `extensions/issues/api`.
- [x] Update `Cargo.toml` and WIT binding paths.
- [x] Test local WASM build and loading.

### Phase 3: Astro Integration Creation ✅ COMPLETE
- [x] Create `extensions/issues/ui` package.
- [x] Implement Astro integration entry point (`index.ts`).
- [x] Set up `graphql-codegen` and create the initial `client.ts`.
- [x] Create Vue components and Astro pages, using the generated SDK.

### Phase 4: Integration & Testing ✅ COMPLETE
- [x] Link integration to `apps/web` for end-to-end testing.
- [x] Write unit and E2E tests (19 unit tests, 10 E2E tests).
- [x] Test the full OCI fetch and installation flow.

### Phase 5: CI/CD & Documentation ✅ COMPLETE
- [x] Create GitHub Actions workflows for building and publishing WASM and npm packages.
- [x] Write developer guides for creating extensions and integrations (ADR-0004, creating-extensions.md, context-versioning.md).

### Completed Deliverables

**Core Infrastructure:**
- Extension package structure at `extensions/issues/{api,shared,ui}`
- WIT interface at `packages/wit/extension.wit` (version 0.2.0)
- Monorepo configuration with workspace support
- GraphQL federation support via Hive Router

**Issues Extension (Reference Implementation):**
- WASM backend with complete GraphQL schema and resolvers
- Astro integration with Vue 3 components
- Type-safe GraphQL client via codegen
- Slot system integration for repository tabs

**CI/CD:**
- `build-extensions.yml` - Builds and publishes WASM to OCI
- `build-integrations.yml` - Builds and publishes UI to npm
- `validate-graphql-schema.yml` - Schema validation

**Testing:**
- 19 unit tests for slot system (all passing)
- 10 E2E tests for issues extension (Playwright)
- Integration tests for extension loading

**Documentation:**
- PRD-0002: This document
- ADR-0004: Extension Slot System
- Guide: Creating Extensions (646 lines)
- Guide: Context Versioning (255 lines)
- AGENTS.md: Copilot agent setup

## Future Work

1. **Extension Marketplace**: UI for discovering and managing extensions.
2. **Hot Reloading**: Reload extensions without a server restart.
3. **Extension CLI**: `forge extension create/build/publish` commands.
4. **Shared WASM Components**: Use the WASM component model for shared libraries between extensions.

## References

- [ADR-0002: WASM Extension System](../adrs/0002-wasm-extension-system.md)
- [ADR-0003: OCI Extension Distribution](../adrs/0003-oci-extension-distribution.md)
- [Astro Integration API](https://docs.astro.build/en/reference/integrations-reference/)
- [GraphQL Code Generator](https://the-guild.dev/graphql/codegen)
