# Extensions

Feature extensions in Forge now live under `extensions/<feature-name>/`, grouping server, client, and shared assets together.

## Layout

```
extensions/
└── <feature>/
    ├── api/            # Rust → WASM implementation that satisfies the WIT contract
    ├── shared/         # GraphQL schema fragments, fixtures, and cross-cutting assets
    └── ui/             # Astro/Vue integration that surfaces the feature in the web app
```

- The **api** crate compiles to WebAssembly (`wasm32-wasip1`) and implements the interfaces defined in `packages/wit/extension.wit`.
- The **shared** folder holds canonical definitions consumed by both halves (for example `schema.graphql`).
- The **ui** package ships the corresponding Astro integration (`@forgepoint/astro-integration-<feature>`).

Refer to [extensions/issues/](./issues/) for a complete example, including build scripts and documentation for local development, testing, and publication.
