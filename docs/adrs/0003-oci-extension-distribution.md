# ADR 0003: OCI-Based Extension Distribution

## Status

Proposed

## Context

Forge currently loads WASM extensions by scanning a local directory for `.wasm` files at startup. This approach works for local development but has limitations for production deployments and extension distribution:

1. **Distribution Challenge**: No standard mechanism for distributing extensions to users
2. **Version Management**: No versioning or update mechanism for extensions
3. **Authentication**: Cannot distribute private/proprietary extensions securely
4. **Caching**: No content-addressable caching for faster startups
5. **Reproducibility**: Difficult to ensure consistent extension versions across environments

We need a standardized distribution mechanism that supports:
- Remote fetching from registries
- Version pinning and updates
- Authentication for private extensions
- Content-addressable caching
- Offline operation for air-gapped environments

## Decision

We will adopt **OCI (Open Container Initiative) registries** as the distribution mechanism for Forge WASM extensions, configured via **RON (Rusty Object Notation)** configuration files.

### Configuration Format

Extensions will be configured in a `forge.ron` file with the following structure:

```ron
Config(
    extensions: Extensions(
        oci: [
            OciExtension(
                name: "github-integration",
                registry: "ghcr.io",
                image: "forgepoint/extensions/github",
                reference: Tag("v1.0.0"),
            ),
        ],
        local: [
            LocalExtension(
                name: "custom-extension",
                path: "./extensions/custom.wasm",
            ),
        ],
        auth: {
            "ghcr.io": RegistryAuth(
                username_env: Some("GHCR_USERNAME"),
                token_env: Some("GHCR_TOKEN"),
            ),
        },
        settings: Settings(
            cache_dir: Some(".forge/extensions/cache"),
            offline_mode: false,
            verify_checksums: true,
        ),
    ),
)
```

### Key Design Choices

#### 1. **OCI as Distribution Format**

- **Standard Protocol**: Reuse existing OCI tooling (crane, skopeo, docker)
- **Registry Infrastructure**: Leverage existing registries (ghcr.io, Docker Hub, private registries)
- **Metadata Support**: OCI manifests provide versioning, checksums, signatures
- **Ecosystem**: Large ecosystem of tools for building, scanning, signing

#### 2. **RON for Configuration**

- **Rust-Native**: Natural syntax for Rust developers
- **Type-Safe**: Strong typing with enums (`Tag` vs `Digest`)
- **Expressive**: Better than TOML for complex nested structures
- **Tooling**: Native `serde` support, syntax highlighting

#### 3. **Content-Addressed Caching**

- **Cache Key**: SHA256 digest of registry reference and tag
- **Storage**: `.forge/extensions/cache/<digest>.wasm`
- **Metadata**: JSON sidecar files track provenance
- **Immutability**: Digest-based storage prevents tampering

#### 4. **No Extension Dependencies**

Extensions are **completely independent** and load in configuration order. This decision avoids:
- Complex dependency resolution
- Version conflict management
- Circular dependency detection
- Loading order computation

Extensions must be self-contained. If shared functionality is needed, it should be provided by the host (Forge server) through the WIT interface.

#### 5. **Offline Mode**

Two operational modes:

- **Online (default)**: Fail startup if OCI fetch fails (deterministic)
- **Offline**: Use cached extensions, log warnings for missing extensions

This provides flexibility for:
- Production deployments (online, strict)
- Development workflows (offline, permissive)
- Air-gapped environments (offline, pre-populated cache)

#### 6. **Authentication via Environment Variables**

- **Security**: Never store credentials in config files
- **Flexibility**: Support standard credential helpers later
- **Simplicity**: Direct mapping to env vars for MVP

### Loading Flow

```
Startup
  ↓
Read forge.ron
  ↓
For each OCI extension:
  ↓
  Compute cache key (digest)
  ↓
  Cache hit? → Yes → Load from cache
  ↓ No
  Offline mode? → Yes → Skip or fail
  ↓ No
  Fetch from OCI registry (with auth)
  ↓
  Validate WASM magic number
  ↓
  Write to cache
  ↓
For each local extension:
  ↓
  Load from filesystem path
  ↓
Initialize all WASM modules
  ↓
Build merged GraphQL schema
  ↓
Start server
```

### Directory Structure

```
.forge/
  config.ron                        # Configuration
  db/
    forge.db
    <extension>.extension.db
  repos/
  extensions/
    cache/
      sha256_<digest>.wasm          # Cached OCI extensions
      sha256_<digest>.metadata.json # Fetch metadata
```

### Security Model

1. **Checksum Verification**: Always verify WASM integrity
2. **Digest Pinning**: Support `Digest("sha256:...")` for immutability
3. **Sandboxing**: WASM modules run in Wasmtime sandbox (existing)
4. **Authentication**: Support private registries via env vars
5. **Validation**: Validate WASM structure before loading

Future enhancements:
- Signature verification (cosign/sigstore)
- Content trust policies
- Audit logging

## Consequences

### Positive

- **Standardization**: Reuse OCI ecosystem and tooling
- **Versioning**: First-class version management with tags/digests
- **Security**: Authentication, checksums, and future signing support
- **Caching**: Fast startup after initial fetch
- **Flexibility**: Support both OCI and local extensions
- **Offline**: Air-gapped deployments supported

### Negative

- **Complexity**: Introduces OCI client dependency and network operations
- **Startup Time**: First fetch adds latency to cold starts
- **Cache Management**: Need cache pruning strategy (manual for MVP)
- **Registry Dependency**: Requires registry availability (mitigated by cache)

### Neutral

- **Configuration Format**: RON is less common than TOML (but more expressive)
- **No Dependencies**: Extensions must be self-contained (simplifies system)

## Alternatives Considered

### 1. **HTTP(S) Direct Download**

```
❌ Rejected

Pros:
- Simpler implementation (just HTTP GET)
- No special tooling required

Cons:
- No standard for metadata, versioning, authentication
- Would need to invent our own manifest format
- No ecosystem tooling (building, scanning, signing)
```

### 2. **Git Submodules**

```
❌ Rejected

Pros:
- Native git integration
- Familiar to developers

Cons:
- Requires build step (source, not compiled WASM)
- Slower startup (compile on every start)
- Not suitable for binary distribution
```

### 3. **Custom Package Registry**

```
❌ Rejected

Pros:
- Full control over format and features

Cons:
- Reinventing the wheel
- No existing ecosystem
- Users need to run their own registry
```

### 4. **TOML Configuration**

```
❌ Rejected (for RON)

Pros:
- More common format
- Simpler syntax

Cons:
- Less expressive for Rust types (enums, complex nesting)
- Harder to represent Tag vs Digest distinction
- Less natural for Rust developers
```

## Implementation Plan

### Phase 1: Configuration Infrastructure
- Add `ron` crate dependency
- Create `crates/server/src/config/` module
- Implement config types with `serde`
- Config file loader with defaults

### Phase 2: OCI Client
- Add `oci-distribution`, `sha2` dependencies
- Implement `OciExtensionFetcher`
- Content-addressed cache with metadata
- Authentication support

### Phase 3: Integration
- Modify `ExtensionManager::load_extensions()`
- Update `main.rs` to load config
- Structured logging for fetch operations

### Phase 4: Error Handling
- Offline mode implementation
- Retry logic and timeouts
- Comprehensive error messages

### Phase 5: Documentation
- Example `forge.ron`
- User guide for OCI extensions
- Extension developer guide

## Future Work

1. **Cache Management**: Automatic pruning, size limits
2. **Signature Verification**: cosign/sigstore integration
3. **CLI Commands**: `forge extensions list/fetch/update/prune`
4. **Hot Reload**: Load extensions without server restart
5. **Credential Helpers**: Support docker-credential-* helpers
6. **Metrics**: Track fetch duration, cache hit ratio

## References

- [OCI Distribution Spec](https://github.com/opencontainers/distribution-spec)
- [OCI Image Spec](https://github.com/opencontainers/image-spec)
- [RON Specification](https://github.com/ron-rs/ron)
- [ADR 0002: WASM Extension System](./0002-wasm-extension-system.md)
