# ADR-0004: Extension Slot System for Frontend UI Injection

- Status: Accepted
- Date: 2025-09-30
- Authors: Forgepoint Dev Team
- Related: ADR-0002, ADR-0003, PRD-0002

## Context

The extension system (ADR-0002, ADR-0003) allows extending the GraphQL API via WASM modules distributed through OCI registries. However, extensions that provide UI functionality need a way to inject components into existing core pages without modifying the core application code.

### Requirements

1. **Non-invasive**: Extensions must inject UI without forking or patching core code
2. **Type-safe**: Context data passed to extension components must be strongly typed
3. **Composable**: Multiple extensions should be able to contribute to the same slot
4. **Ordering**: Extensions must control the display order of their contributions
5. **Build-time Resolution**: Slot contents should be resolved at build time for optimal performance

### Example Use Cases

- **Issues Extension**: Add an "Issues" tab to repository pages
- **CI/CD Extension**: Add a "Pipelines" tab to repository pages
- **Activity Extension**: Add an activity feed widget to the homepage
- **Metrics Extension**: Add visualization tabs to group pages

## Decision

### 1. Slot-Based Architecture

We adopt a slot-based system where core pages define named "slots" that extensions can contribute components to. This pattern is inspired by Astro's component slots and WordPress plugin hooks.

**Supported Slot Types:**

- `repo-tabs`: Add tabs to repository detail pages
- `group-tabs`: Add tabs to group detail pages
- `homepage-widgets`: Add widgets to the forge homepage

### 2. Vite Virtual Modules for Slot Registry

Extensions register slots via Astro integrations, and a Vite plugin aggregates registrations into virtual modules that components import.

**Architecture:**

```
┌─────────────────────┐
│ Astro Integration   │
│ (Extension Package) │
└──────────┬──────────┘
           │ registers slot
           ↓
┌─────────────────────┐
│  Slot Registry      │
│  (Build-time)       │
└──────────┬──────────┘
           │ generates virtual module
           ↓
┌─────────────────────┐
│  virtual:forge/     │
│  slots/repo-tabs    │
└──────────┬──────────┘
           │ imported by
           ↓
┌─────────────────────┐
│  ExtensionTabs.vue  │
│  (Core Component)   │
└─────────────────────┘
```

### 3. Implementation Details

#### Slot Registry (Build-time)

```typescript
// apps/web/src/lib/slot-plugin.ts
export interface SlotDefinition {
  id: string;           // Unique identifier
  label: string;        // Display label (for tabs)
  componentPath: string; // Path to Vue component
  order?: number;       // Display order (default: 0)
}

export function createSlotRegistry() {
  return {
    repoTabs: [],
    groupTabs: [],
    homepageWidgets: [],
  };
}
```

#### Vite Plugin for Virtual Modules

The Vite plugin generates virtual modules at build time:

```typescript
export function createSlotPlugin(registry: SlotRegistry): Plugin {
  return {
    name: 'forge-slot-plugin',
    resolveId(id) {
      if (id === 'virtual:forge/slots/repo-tabs') {
        return '\0' + id;
      }
    },
    load(id) {
      if (id === '\0virtual:forge/slots/repo-tabs') {
        const sorted = [...registry.repoTabs].sort(
          (a, b) => (a.order ?? 0) - (b.order ?? 0)
        );
        const imports = sorted.map(
          (slot, idx) => `import Component${idx} from '${slot.componentPath}';`
        ).join('\n');
        const slots = sorted.map((slot, idx) => `{
          id: '${slot.id}',
          label: '${slot.label}',
          component: Component${idx},
          order: ${slot.order ?? 0}
        }`).join(',\n');
        return `${imports}\n\nexport const repoTabs = [${slots}];`;
      }
    },
  };
}
```

#### Extension Registration

Extensions register slots through Astro integration hooks:

```typescript
// packages/integrations/issues/src/index.ts
export default function issuesIntegration(options) {
  return {
    name: '@forgepoint/astro-integration-issues',
    hooks: {
      'astro:config:setup': ({ injectRoute }) => {
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

#### Core Component Usage

Core pages import virtual modules and render slots:

```vue
<!-- ExtensionTabs.vue -->
<script setup lang="ts">
import { repoTabs } from 'virtual:forge/slots/repo-tabs';

const props = defineProps<{
  repository: RepositoryContext;
}>();
</script>

<template>
  <div v-for="slot in repoTabs" :key="slot.id">
    <component :is="slot.component" :repository="repository" />
  </div>
</template>
```

### 4. Context Type System

Each slot type has a strongly-typed context interface:

```typescript
// apps/web/src/lib/slots.ts
export interface RepositoryContext {
  id: string;
  slug: string;
  fullPath: string;
  isRemote: boolean;
  remoteUrl: string | null;
}

export interface GroupContext {
  id: string;
  slug: string;
  fullPath: string;
}
```

Extension components receive context as props:

```vue
<script setup lang="ts">
import type { RepositoryContext } from 'forge-web/lib/slots';

const props = defineProps<{
  repository: RepositoryContext;
}>();

// Load issues for this repository
const issues = await loadIssues(props.repository.id);
</script>
```

### 5. Configuration

Users configure the slot system in their Astro config:

```javascript
// apps/web/astro.config.mjs
import { createSlotRegistry, createSlotPlugin } from './src/lib/slot-plugin.ts';
import issuesIntegration from '@forgepoint/astro-integration-issues';

const slotRegistry = createSlotRegistry();
const slotPlugin = createSlotPlugin(slotRegistry);

export default defineConfig({
  integrations: [
    vue(),
    issuesIntegration({ slotRegistry }),
  ],
  vite: {
    plugins: [slotPlugin],
  },
});
```

## Consequences

### Positive

1. **Zero Core Changes**: Extensions add UI without modifying core application code
2. **Type Safety**: TypeScript enforces context interface contracts
3. **Performance**: Virtual modules resolved at build time (no runtime overhead)
4. **Developer Experience**: Clear patterns with IDE autocomplete for slot types
5. **HMR Support**: Changes to extension components trigger hot module replacement
6. **Composability**: Multiple extensions can contribute to the same slot type

### Negative

1. **Build-time Only**: Slots must be registered during build (no dynamic runtime loading)
2. **Restart Required**: Adding new integrations requires Astro dev server restart
3. **Tight Coupling**: Extensions depend on core context interface stability
4. **Limited Extensibility**: Only predefined slot types are supported

### Mitigations

1. **Context Versioning**: Add version field to context interfaces, document breaking change policy
2. **Graceful Degradation**: Core pages handle empty slot arrays gracefully
3. **Documentation**: Provide clear guides for slot usage and context contracts
4. **Error Handling**: Validate slot registrations and provide helpful error messages

## Security Considerations

1. **Component Isolation**: Extension components run in the same Vue context as core (no sandbox)
2. **XSS Protection**: Extensions must sanitize user input (same as core components)
3. **Context Data**: Only expose necessary data in context interfaces (principle of least privilege)
4. **Component Paths**: Validate component paths during build to prevent path traversal

## Testing Strategy

1. **Unit Tests**: Test slot registry and virtual module generation
2. **Integration Tests**: Test slot registration from Astro integrations
3. **E2E Tests**: Verify extension tabs appear on correct pages with correct context
4. **Type Tests**: Use `tsd` to validate context interface contracts

## Future Enhancements

1. **Dynamic Slots**: Support runtime slot registration (requires different architecture)
2. **Slot Events**: Allow slots to emit events for inter-extension communication
3. **Conditional Rendering**: Support predicate functions for conditional slot display
4. **Nested Slots**: Allow slots to define sub-slots for deeper composition

## References

- [Astro Integration API](https://docs.astro.build/en/reference/integrations-reference/)
- [Vite Virtual Modules](https://vitejs.dev/guide/api-plugin.html#virtual-modules-convention)
- [Vue 3 Dynamic Components](https://vuejs.org/guide/essentials/component-basics.html#dynamic-components)
- [PRD-0002: Extension Package Architecture](../prds/0002-extension-packages.md)

## Alternatives Considered

### 1. Runtime Slot Registration

**Approach**: Extensions register slots via global registry at runtime

**Rejected because:**
- Adds runtime overhead for slot lookup
- Complicates HMR and dev experience
- Harder to type-check

### 2. Core Page Modification

**Approach**: Extensions provide patches/diffs to core pages

**Rejected because:**
- Fragile and error-prone
- Breaks on core updates
- Difficult to compose multiple extensions

### 3. Component Wrapper Pattern

**Approach**: Extensions wrap core components

**Rejected because:**
- Requires deep knowledge of core component structure
- Tight coupling to implementation details
- Doesn't support multiple extensions

### 4. Web Components / Custom Elements

**Approach**: Extensions register custom elements

**Rejected because:**
- Inconsistent with Vue architecture
- Complicates state management and styling
- Limited TypeScript support for context
