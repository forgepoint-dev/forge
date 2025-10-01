set shell := ["bash", "-cu"]

default: server

# Run the forge server with configurable SQLite and repository roots.
server FORGE_DB_PATH='./.forge/db' FORGE_REPOS_PATH='./.forge/repos':
    mkdir -p {{FORGE_DB_PATH}}
    mkdir -p {{FORGE_REPOS_PATH}}
    FORGE_DB_PATH={{FORGE_DB_PATH}} FORGE_REPOS_PATH={{FORGE_REPOS_PATH}} \
        cargo run --manifest-path server/Cargo.toml --bin server

# Build the forgepoint-extension-issues API for wasm32-wasip1 target.
extension-issues-api-wasm:
    RUSTFLAGS="" cargo build --package forgepoint-extension-issues --target wasm32-wasip1 --release
