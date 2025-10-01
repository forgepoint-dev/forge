# Creating Extensions for Forgepoint

This guide walks you through creating a complete extension for Forgepoint with both backend (WASM) and frontend (Astro integration) components.

## Prerequisites

- Nix development environment: `nix develop --impure`
- Forgepoint server running: `FORGE_IN_MEMORY_DB=true cargo run --bin server`
- Understanding of Rust, WebAssembly, TypeScript, and Vue 3

## Extension Architecture

A complete extension consists of two packages:

1. **WASM Extension** (`packages/extensions/your-extension/`): Rust code compiled to WebAssembly, extends the GraphQL API
2. **Astro Integration** (`packages/integrations/your-extension/`): TypeScript/Vue components, extends the frontend UI

Both packages are versioned independently and distributed through different channels (OCI for WASM, npm for frontend).

## Part 1: Create the WASM Extension

### Step 1: Create the Package Structure

```bash
cd packages/extensions
mkdir my-feature
cd my-feature
```

### Step 2: Create Cargo.toml

```toml
[package]
name = "forgepoint-extension-my-feature"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
# WIT bindings for the extension interface
wit-bindgen = "0.39"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[package.metadata.component]
package = "forgepoint:extension"

[package.metadata.component.target]
path = "../../wit/extension.wit"
```

### Step 3: Implement the Extension

Create `src/lib.rs`:

```rust
use wit_bindgen::generate;

// Generate bindings from the WIT interface
generate!({
    path: "../../wit/extension.wit",
    world: "extension",
});

// Export the implementation
export!(MyFeatureExtension);

struct MyFeatureExtension;

impl Guest for MyFeatureExtension {
    fn initialize(config: String) -> Result<(), String> {
        // Initialize extension state, create database tables, etc.
        let db = forge::extension::database::open()?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS my_items (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
            &[],
        )?;

        Ok(())
    }

    fn provide_schema() -> Result<String, String> {
        Ok(r#"
            type MyItem {
                id: ID!
                name: String!
                createdAt: String!
            }

            extend type Query {
                getAllMyItems: [MyItem!]!
                getMyItem(id: ID!): MyItem
            }

            extend type Mutation {
                createMyItem(name: String!): MyItem!
            }
        "#.to_string())
    }

    fn resolve_field(
        field_name: String,
        args: String,
    ) -> Result<String, String> {
        match field_name.as_str() {
            "getAllMyItems" => get_all_items(),
            "getMyItem" => {
                let args: GetItemArgs = serde_json::from_str(&args)
                    .map_err(|e| format!("Invalid args: {}", e))?;
                get_item(args.id)
            }
            "createMyItem" => {
                let args: CreateItemArgs = serde_json::from_str(&args)
                    .map_err(|e| format!("Invalid args: {}", e))?;
                create_item(args.name)
            }
            _ => Err(format!("Unknown field: {}", field_name)),
        }
    }

    fn shutdown() -> Result<(), String> {
        // Cleanup resources
        Ok(())
    }
}

#[derive(serde::Deserialize)]
struct GetItemArgs {
    id: String,
}

#[derive(serde::Deserialize)]
struct CreateItemArgs {
    name: String,
}

#[derive(serde::Serialize)]
struct MyItem {
    id: String,
    name: String,
    created_at: i64,
}

fn get_all_items() -> Result<String, String> {
    let db = forge::extension::database::open()?;
    let mut stmt = db.prepare("SELECT id, name, created_at FROM my_items")?;
    let items: Vec<MyItem> = stmt
        .query_map([], |row| {
            Ok(MyItem {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    serde_json::to_string(&items)
        .map_err(|e| format!("Serialization error: {}", e))
}

fn get_item(id: String) -> Result<String, String> {
    let db = forge::extension::database::open()?;
    let item = db.query_row(
        "SELECT id, name, created_at FROM my_items WHERE id = ?1",
        &[&id],
        |row| {
            Ok(MyItem {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        },
    )?;

    serde_json::to_string(&item)
        .map_err(|e| format!("Serialization error: {}", e))
}

fn create_item(name: String) -> Result<String, String> {
    let db = forge::extension::database::open()?;
    let id = format!("item_{}", generate_id());
    let created_at = current_timestamp();

    db.execute(
        "INSERT INTO my_items (id, name, created_at) VALUES (?1, ?2, ?3)",
        &[&id, &name, &created_at],
    )?;

    let item = MyItem {
        id,
        name,
        created_at,
    };

    serde_json::to_string(&item)
        .map_err(|e| format!("Serialization error: {}", e))
}

fn generate_id() -> String {
    // Implement ID generation (e.g., UUID)
    "12345".to_string()
}

fn current_timestamp() -> i64 {
    // Get current Unix timestamp
    0
}
```

### Step 4: Create Build Configuration

Create `justfile`:

```makefile
# Build the extension
build:
    cargo build --target wasm32-wasip1 --release

# Run tests
test:
    cargo test

# Publish to OCI registry (requires authentication)
publish version:
    #!/usr/bin/env bash
    set -euo pipefail
    just build
    WASM_FILE="target/wasm32-wasip1/release/forgepoint_extension_my_feature.wasm"
    docker buildx build \
        --platform wasi/wasm \
        --provenance=false \
        --output type=oci,dest=my-feature-{{version}}.tar \
        --file - . <<EOF
    FROM scratch
    COPY ${WASM_FILE} /extension.wasm
    EOF
    echo "Built OCI archive: my-feature-{{version}}.tar"
    echo "Push with: crane push my-feature-{{version}}.tar ghcr.io/you/extensions/my-feature:{{version}}"
```

### Step 5: Build and Test

```bash
just build
just test
```

The WASM module will be at `target/wasm32-wasip1/release/forgepoint_extension_my_feature.wasm`.

### Step 6: Local Testing

Add to `forge.ron`:

```ron
Config(
    extensions: Extensions(
        local: [
            LocalExtension(
                name: "my-feature",
                path: "./packages/extensions/my-feature/target/wasm32-wasip1/release/forgepoint_extension_my_feature.wasm",
            ),
        ],
    ),
)
```

Restart the server and test queries in GraphQL playground:

```graphql
mutation {
  createMyItem(name: "Test Item") {
    id
    name
    createdAt
  }
}

query {
  getAllMyItems {
    id
    name
    createdAt
  }
}
```

## Part 2: Create the Astro Integration

### Step 1: Create Package Structure

```bash
cd packages/integrations
mkdir my-feature
cd my-feature
bun init -y
```

### Step 2: Configure package.json

```json
{
  "name": "@forgepoint/astro-integration-my-feature",
  "version": "0.1.0",
  "type": "module",
  "exports": {
    ".": "./src/index.ts",
    "./components/*": "./src/components/*"
  },
  "scripts": {
    "codegen": "graphql-codegen -c codegen.ts",
    "dev": "bun run codegen --watch"
  },
  "dependencies": {
    "astro": "^4.0.0",
    "vue": "^3.4.0"
  },
  "devDependencies": {
    "@graphql-codegen/cli": "^5.0.0",
    "@graphql-codegen/client-preset": "^4.0.0",
    "@graphql-codegen/typescript": "^4.0.0",
    "@graphql-codegen/typescript-operations": "^4.0.0",
    "typescript": "^5.3.0"
  },
  "peerDependencies": {
    "astro": "^4.0.0"
  }
}
```

### Step 3: Configure GraphQL Code Generator

Create `codegen.ts`:

```typescript
import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
  schema: 'http://localhost:8000/graphql',
  documents: ['src/**/*.{ts,tsx,vue}'],
  generates: {
    './src/lib/generated/': {
      preset: 'client',
      plugins: [],
      config: {
        useTypeImports: true,
      },
    },
  },
};

export default config;
```

### Step 4: Create Type-Safe GraphQL Client

Create `src/lib/client.ts`:

```typescript
import { getSdk } from './generated/graphql';
import { graphqlRequest } from 'forge-web/lib/graphql';

const client = async <TData, TVariables>(
  query: string,
  variables?: TVariables
): Promise<TData> => {
  return graphqlRequest<TData>({ query, variables });
};

export const sdk = getSdk(client);
```

### Step 5: Create GraphQL Operations

Create `src/lib/queries.ts`:

```typescript
import { graphql } from './generated';

export const GET_ALL_MY_ITEMS = graphql(`
  query GetAllMyItems {
    getAllMyItems {
      id
      name
      createdAt
    }
  }
`);

export const GET_MY_ITEM = graphql(`
  query GetMyItem($id: ID!) {
    getMyItem(id: $id) {
      id
      name
      createdAt
    }
  }
`);

export const CREATE_MY_ITEM = graphql(`
  mutation CreateMyItem($name: String!) {
    createMyItem(name: $name) {
      id
      name
      createdAt
    }
  }
`);
```

### Step 6: Create Vue Components

Create `src/components/MyItemsList.vue`:

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { sdk } from '../lib/client';
import type { GetAllMyItemsQuery } from '../lib/generated/graphql';

const items = ref<GetAllMyItemsQuery['getAllMyItems']>([]);
const loading = ref(true);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    const response = await sdk.getAllMyItems();
    items.value = response.getAllMyItems;
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load items';
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="my-items-list">
    <div v-if="loading" class="text-muted-foreground">
      Loading items...
    </div>
    <div v-else-if="error" class="text-destructive">
      {{ error }}
    </div>
    <ul v-else class="space-y-2">
      <li v-for="item in items" :key="item.id" class="p-3 border rounded">
        <a :href="`/my-feature/${item.id}`" class="font-medium hover:underline">
          {{ item.name }}
        </a>
        <p class="text-sm text-muted-foreground">
          Created: {{ new Date(parseInt(item.createdAt)).toLocaleDateString() }}
        </p>
      </li>
    </ul>
  </div>
</template>
```

Create `src/components/MyFeatureTab.vue` (for repository slot):

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import type { RepositoryContext } from 'forge-web/lib/slots';
import { sdk } from '../lib/client';

const props = defineProps<{
  repository: RepositoryContext;
}>();

const items = ref([]);
const loading = ref(true);

onMounted(async () => {
  try {
    // Filter items by repository
    const response = await sdk.getAllMyItems();
    items.value = response.getAllMyItems.filter(
      item => item.repositoryId === props.repository.id
    );
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="p-5">
    <h3 class="text-lg font-semibold mb-4">
      My Feature Items for {{ repository.slug }}
    </h3>
    <div v-if="loading">Loading...</div>
    <MyItemsList v-else :items="items" />
  </div>
</template>
```

### Step 7: Create Astro Pages

Create `src/pages/MyItemList.astro`:

```astro
---
import MyItemsList from '../components/MyItemsList.vue';
---

<html>
  <head>
    <title>My Feature Items</title>
  </head>
  <body>
    <h1>My Feature Items</h1>
    <MyItemsList client:load />
  </body>
</html>
```

### Step 8: Create Integration Entry Point

Create `src/index.ts`:

```typescript
import type { AstroIntegration } from 'astro';

export interface MyFeatureIntegrationOptions {
  slotRegistry?: {
    repoTabs: Array<{
      id: string;
      label: string;
      componentPath: string;
      order?: number;
    }>;
  };
}

export default function myFeatureIntegration(
  options: MyFeatureIntegrationOptions = {}
): AstroIntegration {
  return {
    name: '@forgepoint/astro-integration-my-feature',
    hooks: {
      'astro:config:setup': ({ injectRoute }) => {
        // Inject standalone routes
        injectRoute({
          pattern: '/my-feature',
          entrypoint: '@forgepoint/astro-integration-my-feature/pages/MyItemList.astro',
        });

        // Register slot if registry provided
        if (options.slotRegistry) {
          options.slotRegistry.repoTabs.push({
            id: 'my-feature',
            label: 'My Feature',
            componentPath: '@forgepoint/astro-integration-my-feature/components/MyFeatureTab.vue',
            order: 20,
          });
        }
      },
    },
  };
}
```

### Step 9: Create README

Create `README.md` with installation and usage instructions (see `packages/integrations/issues/README.md` as template).

### Step 10: Generate Types and Test

```bash
bun install
bun run codegen
```

## Part 3: Integration Testing

### Step 1: Link Integration to Main App

In `apps/web/astro.config.mjs`:

```javascript
import myFeatureIntegration from '@forgepoint/astro-integration-my-feature';

export default defineConfig({
  integrations: [
    vue(),
    myFeatureIntegration({ slotRegistry }),
  ],
});
```

### Step 2: Test Locally

1. Start the server: `FORGE_IN_MEMORY_DB=true cargo run --bin server`
2. Start Astro: `cd apps/web && bun run dev`
3. Navigate to repository pages - you should see "My Feature" tab
4. Navigate to `/my-feature` - you should see the standalone page

## Part 4: Publishing

### Publish WASM to OCI Registry

```bash
cd packages/extensions/my-feature
just publish 0.1.0
crane push my-feature-0.1.0.tar ghcr.io/your-org/extensions/my-feature:v0.1.0
```

### Publish Integration to npm

```bash
cd packages/integrations/my-feature
npm publish --access public
```

## Best Practices

1. **Error Handling**: Always validate inputs and handle errors gracefully
2. **Type Safety**: Use generated types from GraphQL codegen
3. **Performance**: Use database indexes for queries
4. **Security**: Sanitize user input, don't expose sensitive data in context
5. **Testing**: Write tests for both WASM and Vue components
6. **Documentation**: Include README with clear usage instructions

## Troubleshooting

**Issue**: GraphQL codegen fails with "Cannot query field"
- **Solution**: Ensure server is running and extension is loaded

**Issue**: Component not appearing in slot
- **Solution**: Check slotRegistry is passed to integration, verify component path

**Issue**: WASM build fails
- **Solution**: Ensure `wasm32-wasip1` target installed: `rustup target add wasm32-wasip1`

## Next Steps

- Read [ADR-0004](../adrs/0004-extension-slot-system.md) for slot system details
- Review [Issues Extension](../../packages/integrations/issues/) as reference
- Check [PRD-0002](../prds/0002-extension-packages.md) for architecture overview
