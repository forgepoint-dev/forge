# Issues Extension

The Issues extension ships both the GraphQL/WASM backend and the Astro UI integration. Components are grouped by responsibility inside this feature directory:

```
extensions/issues/
├── api/            # Rust crate compiled to WebAssembly
├── shared/         # Canonical GraphQL schema and other cross-cutting assets
└── ui/             # Astro/Vue integration published to npm
```

## API (Rust → WASM)

- Crate name: `forgepoint-extension-issues`
- Target: `wasm32-wasip1`
- Entry point: `extensions/issues/api/src/lib.rs`

Common commands (requires `wasm32-wasip1` target installed):

```bash
cd extensions/issues/api
cargo build --target wasm32-wasip1 --release
cargo test
cargo fmt -- --check
cargo clippy --target wasm32-wasip1 -- -D warnings
```

To copy the compiled module next to the server for local testing:

```bash
just install-local
```

## Shared Assets

`extensions/issues/shared/schema.graphql` contains the GraphQL schema fragment. It is loaded at compile time by the Rust crate and reused by the UI codegen step to ensure both halves stay in sync.

## UI (Astro Integration)

- Package name: `@forgepoint/astro-integration-issues`
- Entry point: `extensions/issues/ui/src/index.ts`

Development commands (requires Bun 1.1.30):

```bash
cd extensions/issues/ui
bun install
bun run codegen
bun test
```

In local development the web app consumes the workspace source directly:

```javascript
// apps/web/astro.config.mjs
import issuesIntegration from "../../extensions/issues/ui/src/index.ts";

export default defineConfig({
  integrations: [issuesIntegration()],
});
```

For production builds publish the package to npm and depend on the published version instead.

## Publishing

### WebAssembly crate → OCI registry

```
cd extensions/issues/api
just publish <version>
```

### Astro integration → npm

```
cd extensions/issues/ui
npm publish --access public
```

See [docs/guides/creating-extensions.md](../../docs/guides/creating-extensions.md) for the full authoring workflow.
