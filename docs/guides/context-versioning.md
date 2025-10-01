# Extension Context Versioning

This guide explains the versioning strategy for extension slot context interfaces to maintain backward compatibility.

## Overview

Extension slots receive context objects as props (e.g., `RepositoryContext`, `GroupContext`). These interfaces may evolve over time as new features are added. To prevent breaking existing extensions, we use explicit versioning.

## Version Field

All context interfaces include a `version` field:

```typescript
export interface RepositoryContext {
  version: 1;
  id: string;
  slug: string;
  fullPath: string;
  isRemote: boolean;
  remoteUrl: string | null;
}
```

## Breaking vs Non-Breaking Changes

### Non-Breaking Changes (Patch/Minor Version Bump)

These changes DO NOT require a version increment:

- **Adding optional fields**: New fields with `?` or `| null` type
- **Widening types**: Making a field accept more values (e.g., `string` → `string | number`)
- **Documentation updates**: Changes to JSDoc comments

**Example:**
```typescript
export interface RepositoryContext {
  version: 1;
  id: string;
  slug: string;
  fullPath: string;
  isRemote: boolean;
  remoteUrl: string | null;
  createdAt?: string; // ✅ New optional field, no version bump
}
```

### Breaking Changes (Major Version Bump)

These changes REQUIRE incrementing the `version` field:

- **Removing fields**: Deleting any property
- **Renaming fields**: Changing property names
- **Narrowing types**: Making a field accept fewer values
- **Making fields required**: Changing `optional?` to required

**Example:**
```typescript
export interface RepositoryContext {
  version: 2; // ⚠️ Version bumped due to breaking change
  id: string;
  slug: string;
  path: string; // ⚠️ Renamed from fullPath
  isRemote: boolean;
  remoteUrl: string | null;
}
```

## Implementation Strategy

### 1. Type Guards for Version Checking

Extensions should use type guards to handle different versions:

```typescript
// Extension component checking version
function isV1Context(ctx: RepositoryContext): ctx is RepositoryContextV1 {
  return ctx.version === 1;
}

function isV2Context(ctx: RepositoryContext): ctx is RepositoryContextV2 {
  return ctx.version === 2;
}

// Usage in component
if (isV1Context(props.repository)) {
  // Use fullPath
  console.log(props.repository.fullPath);
} else if (isV2Context(props.repository)) {
  // Use path
  console.log(props.repository.path);
}
```

### 2. Core Compatibility Layer

The core application supports multiple versions simultaneously:

```typescript
// apps/web/src/lib/slots.ts
export type RepositoryContext = RepositoryContextV1 | RepositoryContextV2;

export interface RepositoryContextV1 {
  version: 1;
  id: string;
  slug: string;
  fullPath: string;
  isRemote: boolean;
  remoteUrl: string | null;
}

export interface RepositoryContextV2 {
  version: 2;
  id: string;
  slug: string;
  path: string;
  isRemote: boolean;
  remoteUrl: string | null;
}

// Helper to create context for current version
export function createRepositoryContext(repo: Repository): RepositoryContextV1 {
  return {
    version: 1,
    id: repo.id,
    slug: repo.slug,
    fullPath: repo.fullPath,
    isRemote: repo.isRemote,
    remoteUrl: repo.remoteUrl,
  };
}
```

### 3. Deprecation Timeline

When introducing a breaking change:

1. **Announcement (v1.0.0)**: Document upcoming breaking change
2. **Dual Support (v1.1.0 - v1.9.0)**: Both versions supported
3. **Deprecation Warning (v2.0.0-beta)**: Warn when v1 is used
4. **Removal (v2.0.0)**: Drop support for v1

**Example:**
```typescript
// In v1.1.0 - v1.9.0, emit deprecation warning
if (props.repository.version === 1) {
  console.warn(
    'RepositoryContext v1 is deprecated and will be removed in v2.0.0. ' +
    'Please update your extension to use v2 context.'
  );
}
```

## Extension Package Compatibility

Extensions declare compatible context versions in `package.json`:

```json
{
  "name": "@forgepoint/astro-integration-issues",
  "version": "1.0.0",
  "forgepoint": {
    "extensionName": "issues",
    "compatibleExtensionVersions": "^0.1.0",
    "contextVersions": {
      "RepositoryContext": [1, 2],
      "GroupContext": [1]
    }
  }
}
```

## Migration Guide for Extension Authors

### Scenario: RepositoryContext v1 → v2

**Before (v1):**
```vue
<script setup lang="ts">
import type { RepositoryContext } from 'forge-web/lib/slots';

const props = defineProps<{
  repository: RepositoryContext;
}>();

console.log(props.repository.fullPath); // v1 field
</script>
```

**After (v2 with backward compatibility):**
```vue
<script setup lang="ts">
import type { RepositoryContext } from 'forge-web/lib/slots';

const props = defineProps<{
  repository: RepositoryContext;
}>();

const path = computed(() => {
  if (props.repository.version === 1) {
    return props.repository.fullPath; // v1 field
  } else {
    return props.repository.path; // v2 field
  }
});

console.log(path.value);
</script>
```

**After (v2 only, dropping v1 support):**
```vue
<script setup lang="ts">
import type { RepositoryContextV2 } from 'forge-web/lib/slots';

const props = defineProps<{
  repository: RepositoryContextV2;
}>();

console.log(props.repository.path); // v2 field only
</script>
```

Update `package.json`:
```json
{
  "forgepoint": {
    "contextVersions": {
      "RepositoryContext": [2]
    }
  }
}
```

## Best Practices

1. **Avoid Breaking Changes**: Prefer adding optional fields over renaming/removing
2. **Document Changes**: Update CHANGELOG.md with context interface changes
3. **Test Multiple Versions**: Test extensions with both old and new context versions during transition
4. **Gradual Migration**: Give extensions at least 6 months to migrate before removing old versions
5. **Runtime Validation**: Use Zod or similar to validate context at runtime during development

## Version History

### Version 1 (2025-09-30)

**Initial context interfaces:**

- `RepositoryContext v1`: id, slug, fullPath, isRemote, remoteUrl
- `GroupContext v1`: id, slug, fullPath

## Future Considerations

- **Semantic Versioning**: Context versions follow semantic versioning principles
- **Automated Compatibility Checks**: CI/CD validates extension compatibility declarations
- **Type-Level Version Checking**: Use TypeScript conditional types for compile-time version checking
