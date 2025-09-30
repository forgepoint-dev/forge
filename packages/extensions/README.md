# Extensions

This directory contains WASM extension implementations for Forge.

Each extension is a Rust project that compiles to WebAssembly and implements
the WIT interface defined in `packages/wit/extension.wit`.

## Structure

```
packages/extensions/
└── issues/              # Example: Issues extension
    ├── Cargo.toml      # Rust package configuration
    ├── src/
    │   └── lib.rs      # Extension implementation
    └── justfile        # Build and publish commands
```

## Building Extensions

Extensions target `wasm32-wasip1`:

```bash
cd packages/extensions/issues
cargo build --target wasm32-wasip1 --release
```

## Distribution

Extensions are published to OCI registries (e.g., GitHub Container Registry)
and referenced in `forge.ron` configuration.

See [ADR-0003](../../docs/adrs/0003-oci-extension-distribution.md) for details.
