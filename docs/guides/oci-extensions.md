# OCI Extension Distribution Guide

This guide explains how to configure and use Forge with OCI-distributed WASM extensions.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Authentication](#authentication)
- [Offline Mode](#offline-mode)
- [Caching](#caching)
- [Publishing Extensions](#publishing-extensions)
- [Troubleshooting](#troubleshooting)

## Overview

Forge supports loading WASM extensions from OCI (Open Container Initiative) registries like GitHub Container Registry, Docker Hub, or private registries. This provides:

- **Version Control**: Pin extensions to specific versions using tags or digests
- **Distribution**: Share extensions publicly or privately
- **Caching**: Fast startup with content-addressable caching
- **Security**: Authentication support for private registries

## Quick Start

### 1. Create Configuration File

Create a `forge.ron` file in your project root:

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
        local: [],
        auth: {},
        settings: Settings(
            cache_dir: Some(".forge/extensions/cache"),
            offline_mode: false,
            verify_checksums: true,
        ),
    ),
)
```

### 2. Run Forge

```bash
cargo run --bin server
```

Forge will:
1. Read the configuration file
2. Fetch the extension from `ghcr.io`
3. Cache it in `.forge/extensions/cache`
4. Load the extension

Subsequent startups will use the cached version (unless you change the reference).

## Configuration

### Configuration File Locations

Forge searches for configuration in the following order:

1. Path specified in `FORGE_CONFIG_PATH` environment variable
2. `forge.ron` in the current directory
3. `.forge/config.ron`

If no configuration is found, Forge falls back to scanning the `crates/server/extensions/` directory for `.wasm` files.

### OCI Extension Configuration

```ron
OciExtension(
    name: "extension-name",      // Used for logging and database naming
    registry: "ghcr.io",          // Registry hostname
    image: "org/repo/extension",  // Image path within registry
    reference: Tag("v1.0.0"),     // Version reference
)
```

### Reference Types

#### Tags (Mutable)

```ron
reference: Tag("v1.0.0")
reference: Tag("latest")
```

Tags can point to different content over time. Suitable for development but not recommended for production.

#### Digests (Immutable)

```ron
reference: Digest("sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890")
```

Digests are immutable and ensure exact version reproducibility. **Recommended for production.**

To get a digest:
```bash
crane digest ghcr.io/forgepoint/extensions/github:v1.0.0
```

### Local Extension Configuration

For development, you can load extensions from the filesystem:

```ron
LocalExtension(
    name: "dev-extension",
    path: "./extensions/dev.wasm",  // Relative or absolute path
)
```

### Extension Settings

```ron
settings: Settings(
    // Directory for caching OCI-fetched extensions
    cache_dir: Some(".forge/extensions/cache"),

    // Use cached extensions even if registry is unreachable
    offline_mode: false,

    // Verify SHA256 checksums of cached extensions
    verify_checksums: true,
)
```

## Authentication

### Public Registries

No authentication is required for public registries:

```ron
oci: [
    OciExtension(
        name: "public-extension",
        registry: "ghcr.io",
        image: "org/public-repo/extension",
        reference: Tag("v1.0.0"),
    ),
],
auth: {},  // No authentication needed
```

### Private Registries

For private registries, configure authentication using environment variables:

```ron
auth: {
    "ghcr.io": RegistryAuth(
        username_env: Some("GHCR_USERNAME"),
        token_env: Some("GHCR_TOKEN"),
    ),
    "registry.company.com": RegistryAuth(
        username_env: Some("COMPANY_REGISTRY_USER"),
        token_env: Some("COMPANY_REGISTRY_TOKEN"),
    ),
},
```

Set the environment variables before running Forge:

```bash
export GHCR_USERNAME=your-username
export GHCR_TOKEN=ghp_yourpersonalaccesstoken
```

**Security Note**: Never hardcode credentials in configuration files. Always use environment variables.

### GitHub Container Registry Authentication

1. Create a Personal Access Token at https://github.com/settings/tokens
2. Grant `read:packages` scope
3. Set environment variables:

```bash
export GHCR_USERNAME=your-github-username
export GHCR_TOKEN=ghp_yourtoken
```

## Offline Mode

Offline mode allows Forge to start even when OCI registries are unavailable.

### Enable Offline Mode

```ron
settings: Settings(
    offline_mode: true,
    cache_dir: Some(".forge/extensions/cache"),
    verify_checksums: true,
)
```

### Behavior

- **Online Mode** (default): Forge fails to start if an extension cannot be fetched
- **Offline Mode**: Forge uses cached extensions and logs warnings for missing extensions

### Use Cases

- **Air-Gapped Deployments**: Pre-populate cache, then run offline
- **Development**: Avoid network delays during iteration
- **CI/CD**: Use cached extensions for faster build times

### Pre-Populating Cache

To pre-populate the cache for offline use:

1. Run Forge once in online mode to fetch extensions
2. Copy `.forge/extensions/cache/` to the offline environment
3. Enable `offline_mode: true` in configuration

## Caching

### Cache Structure

Extensions are cached using content-addressable storage:

```
.forge/extensions/cache/
  ├── <sha256-hash>.wasm           # WASM module
  └── <sha256-hash>.metadata.json  # Fetch metadata
```

The cache key is computed from `registry:image@reference`.

### Cache Lifecycle

- **Cache Hit**: Extension is loaded from cache (fast startup)
- **Cache Miss**: Extension is fetched from registry and cached
- **Checksum Verification**: Cached extensions are verified on load (if `verify_checksums: true`)

### Manual Cache Management

**List cached extensions:**
```bash
ls -lh .forge/extensions/cache/
```

**Clear cache:**
```bash
rm -rf .forge/extensions/cache/
```

Forge will re-fetch extensions on next startup.

**Cache size:**
```bash
du -sh .forge/extensions/cache/
```

## Publishing Extensions

### 1. Build WASM Extension

```bash
cd crates/server/extensions/your-extension
cargo build --target wasm32-wasip1 --release
```

### 2. Create OCI Image

Use `crane` or `docker` to push the WASM module:

**Using crane:**
```bash
crane append \
  --new_layer target/wasm32-wasip1/release/your_extension.wasm \
  --new_tag ghcr.io/your-org/extensions/your-extension:v1.0.0
```

**Using docker:**
```dockerfile
# Dockerfile.extension
FROM scratch
COPY target/wasm32-wasip1/release/your_extension.wasm /extension.wasm
```

```bash
docker build -f Dockerfile.extension -t ghcr.io/your-org/extensions/your-extension:v1.0.0 .
docker push ghcr.io/your-org/extensions/your-extension:v1.0.0
```

### 3. Get Digest (Recommended)

```bash
crane digest ghcr.io/your-org/extensions/your-extension:v1.0.0
```

Use this digest in your configuration for immutable deployments.

## Troubleshooting

### Extension Not Found

**Error:**
```
Failed to fetch OCI extension: Failed to pull OCI image
```

**Solutions:**
- Verify the registry, image, and reference are correct
- Check authentication credentials
- Ensure the extension is publicly accessible or credentials are configured

### Authentication Failed

**Error:**
```
Failed to pull OCI image: authentication failed
```

**Solutions:**
- Verify `GHCR_USERNAME` and `GHCR_TOKEN` environment variables are set
- Check token has `read:packages` scope
- Confirm token has not expired

### Checksum Verification Failed

**Error:**
```
Checksum verification failed for <cache-key>, re-fetching
```

**Solutions:**
- Cache corruption detected, Forge will automatically re-fetch
- If issue persists, clear cache: `rm -rf .forge/extensions/cache/`

### Offline Mode: Extension Not Cached

**Warning:**
```
Skipping extension-name (offline mode, not cached)
```

**Solutions:**
- Run Forge in online mode first to populate cache
- Copy cache from another environment
- Disable offline mode to fetch from registry

### Invalid WASM Module

**Error:**
```
Invalid WASM magic number
```

**Solutions:**
- Verify the OCI image contains a valid WASM module
- Ensure the extension was built with `--target wasm32-wasip1`
- Check the image was pushed correctly

### Configuration Parse Error

**Error:**
```
Failed to parse RON configuration
```

**Solutions:**
- Validate RON syntax (trailing commas, matching parentheses)
- Check for typos in field names
- Refer to `forge.example.ron` for correct structure

## Best Practices

### Production Deployments

1. **Use Digests**: Pin extensions to immutable digests, not tags
2. **Enable Checksum Verification**: Keep `verify_checksums: true`
3. **Disable Offline Mode**: Use `offline_mode: false` for deterministic startup
4. **Private Registry**: Host extensions on a private registry

### Development

1. **Use Tags**: Reference extensions by version tags for easy updates
2. **Local Extensions**: Use `local:` for rapid iteration
3. **Enable Offline Mode**: Reduce network delays during development

### Security

1. **Environment Variables**: Never hardcode credentials in `forge.ron`
2. **Minimal Scopes**: Grant only `read:packages` to tokens
3. **Rotate Tokens**: Regularly rotate access tokens
4. **Audit Extensions**: Review extension source code before deployment

## Further Reading

- [ADR 0003: OCI-Based Extension Distribution](../adrs/0003-oci-extension-distribution.md)
- [ADR 0002: WASM Extension System](../adrs/0002-wasm-extension-system.md)
- [OCI Distribution Specification](https://github.com/opencontainers/distribution-spec)
