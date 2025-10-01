# ADR-0006: Frontend Architecture (Astro + Vue 3)

- Status: Accepted
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Context

Forgepoint needs a modern, fast, and extensible web frontend that:
- Displays repository and group hierarchies
- Integrates with the GraphQL API
- Supports dynamic extension-provided UI components
- Provides excellent developer experience
- Delivers fast page loads and good SEO
- Maintains single-user/single-organization focus

The frontend must balance static site generation benefits (fast loads, SEO) with dynamic interactivity (Vue components, real-time API calls).

## Decision

We chose **Astro + Vue 3** with Tailwind CSS as the frontend architecture, with a custom extension slot system for dynamic UI integration.

### 1. Technology Stack

**Core Framework: Astro 5**
- Multi-page application (MPA) architecture for fast navigation
- Server-side rendering (SSR) with partial hydration
- File-based routing (`src/pages/`)
- Excellent performance out-of-the-box
- Built-in TypeScript support

**Component Framework: Vue 3**
- Used for interactive "islands" within Astro pages
- Composition API for better TypeScript integration
- Reactive data binding for GraphQL responses
- Component reusability across pages

**Styling: Tailwind CSS + shadcn-inspired tokens**
- Utility-first CSS framework
- Custom CSS variables for theme consistency
- `applyBaseStyles: false` to manage base styles manually
- shadcn-style design tokens for UI primitives

**Package Manager: Bun 1.1.30**
- Fast dependency installation
- Workspace support for monorepo structure
- Integrated test runner
- Better performance than npm/yarn

### 2. Architecture Patterns

#### Astro Pages (Static Shell)

Pages are Astro components (`*.astro`) that provide the static HTML structure:
```
src/pages/
├── index.astro              # Homepage
├── [...path].astro          # Dynamic repository/group viewer
└── layouts/
    └── MainLayout.astro     # Shared layout
```

**Benefits:**
- Fast initial page load (minimal JavaScript)
- SEO-friendly static HTML
- Shared layouts reduce duplication

#### Vue Islands (Interactive Components)

Interactive functionality is isolated to Vue components:
```
src/components/
├── HomeLanding.vue          # Main homepage component
├── RepoView.vue             # Repository viewer
├── CreateRepositoryModal.vue # Repository creation
├── ExtensionTabs.vue        # Extension slot rendering
└── ui/                      # Reusable UI primitives
    ├── button.vue
    ├── input.vue
    └── modal.vue
```

**Benefits:**
- Component-level hydration (only interactive parts load JavaScript)
- Reactive state management within islands
- Easy to test and maintain

#### GraphQL Integration

Direct GraphQL queries from Vue components:
- Environment variable `PUBLIC_FORGE_GRAPHQL_URL` configures endpoint
- Fetch API for queries and mutations (no heavy client library)
- Type-safe with TypeScript (types generated or manually defined)

### 3. Extension Slot System

A custom Vite plugin provides dynamic UI extension points:

**Slot Types:**
- `repoTabs`: Tabs on repository pages
- `groupTabs`: Tabs on group pages
- `homepageWidgets`: Widgets on the homepage
- `actions`: Action buttons (dashboard or repository scope)

**Implementation (`src/lib/slot-plugin.ts`):**
```typescript
export function createSlotRegistry(): SlotRegistry
export function createSlotPlugin(registry: SlotRegistry): Plugin
```

**How it works:**
1. Extensions register slots via Astro integrations
2. Vite plugin generates virtual modules at build time
3. Components import from `virtual:forge/slots/*`
4. Slots are sorted by `order` and rendered dynamically

**Example Integration:**
```javascript
// astro.config.mjs
import issuesIntegration from '@forgepoint/astro-integration-issues';

export default defineConfig({
  integrations: [
    vue(),
    tailwind(),
    issuesIntegration({ slotRegistry })
  ]
});
```

**Benefits:**
- Extensions can add UI without modifying core code
- Type-safe slot definitions
- Order control via `order` property
- Duplicate detection with warnings

### 4. Project Structure

```
apps/web/
├── src/
│   ├── pages/              # Astro pages (routes)
│   ├── components/         # Vue components
│   ├── layouts/            # Astro layouts
│   └── lib/                # Utilities and plugins
├── public/                 # Static assets
├── tests/                  # E2E and unit tests
├── astro.config.mjs        # Astro configuration
├── tailwind.config.ts      # Tailwind configuration
├── package.json            # Dependencies and scripts
└── tsconfig.json           # TypeScript configuration
```

### 5. Build and Development

**Development:**
```bash
bun run dev
```
- Hot module reloading (HMR)
- Fast refresh for Vue components
- Instant feedback on changes

**Build:**
```bash
bun run build
```
- Static HTML generation for pages
- JavaScript bundling and minification
- CSS optimization and purging

**Testing:**
```bash
bun run test          # Vitest unit tests
bun run test:e2e      # Playwright E2E tests
```

## Consequences

### Positive

- **Performance**: Astro's partial hydration delivers fast page loads
- **Developer Experience**: Vue 3 + TypeScript provides excellent DX
- **Extensibility**: Slot system allows extensions to add UI without core changes
- **Maintainability**: Clear separation between static (Astro) and dynamic (Vue)
- **SEO**: Server-rendered HTML improves discoverability
- **Monorepo Support**: Works seamlessly with Bun workspaces

### Negative

- **Complexity**: Two-framework approach (Astro + Vue) has learning curve
- **State Management**: No global state library (Pinia, Vuex) - may need for complex apps
- **Build Time**: Virtual module generation adds slight build overhead
- **Framework Lock-in**: Astro-specific patterns limit portability

### Trade-offs

- **Astro vs Next.js/Nuxt**: Chose Astro for simplicity and performance
  - Rationale: Single-user forge doesn't need complex server logic or auth
- **Vue vs React**: Chose Vue for better integration with Astro and simpler reactivity
  - Rationale: Vue's template syntax feels natural in Astro islands
- **Custom Slots vs Astro Middleware**: Built custom slot system instead of using middleware
  - Rationale: Type-safe compile-time slot resolution vs runtime hooks
- **No State Library**: Chose component-local state over global state management
  - Rationale: Current UI complexity doesn't justify Pinia/Vuex overhead

## Implementation Details

### File Structure

- `apps/web/src/pages/`: Astro pages with file-based routing
- `apps/web/src/components/`: Vue components for interactivity
- `apps/web/src/lib/slot-plugin.ts`: Virtual module system for extension slots
- `apps/web/astro.config.mjs`: Astro configuration with integrations

### Environment Variables

- `PUBLIC_FORGE_GRAPHQL_URL`: GraphQL endpoint (default: `http://localhost:8000/graphql`)

### Dependencies

**Core:**
- `astro@^5.14.1`
- `@astrojs/vue@^5.0.0`
- `@astrojs/tailwind@^5.0.0`
- `vue@^3.4.38`
- `tailwindcss@^3.4.14`

**Development:**
- `vitest@^2.0.0` - Unit testing
- `@playwright/test@^1.55.1` - E2E testing
- `bun@1.1.30` - Package manager

### Routing

- `/` - Homepage (HomeLanding.vue)
- `/[...path]` - Dynamic repository/group viewer
  - Resolves paths like `/my-group/my-repo`
  - Fetches data from GraphQL API
  - Renders appropriate view (group or repository)

## Future Enhancements

1. **Authentication UI**: Add login/logout flows when ATProto OAuth is implemented
2. **Global State**: Integrate Pinia for complex state management if needed
3. **Code Splitting**: Further optimize bundle size with dynamic imports
4. **Offline Support**: Add service worker for offline browsing
5. **Dark Mode**: Add theme toggle with system preference detection
6. **Accessibility**: Enhance ARIA labels and keyboard navigation
7. **Mobile Optimization**: Improve responsive design for mobile devices
8. **GraphQL Codegen**: Auto-generate TypeScript types from schema

## References

- Astro documentation: https://docs.astro.build
- Vue 3 documentation: https://vuejs.org
- Tailwind CSS: https://tailwindcss.com
- Extension slot system: `apps/web/src/lib/slot-plugin.ts`
- Issues extension integration: `extensions/issues/ui/src/index.ts`
